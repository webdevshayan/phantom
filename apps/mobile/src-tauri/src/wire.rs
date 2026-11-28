//! The wire contract between the native core and the injected chrome.
//!
//! Every snapshot travels as bincode: fixed-width little-endian
//! integers, one byte per bool, a u64 length followed by UTF-8 bytes for
//! each string, a u64 length followed by items for each vector, and
//! struct fields in declaration order. The injected decoder mirrors this
//! layout exactly, and `golden_layout` pins the leading bytes so the two
//! sides cannot drift apart. No JSON crosses the UI boundary in either
//! direction.
//!
//! Layout reference (offsets for a snapshot with empty strings and no
//! tabs): revision 0..8, dark 8, theme 9..49 (ten u32 colors), omnibox
//! length 49..57, can_back, can_forward, tab_count u32, active_tab_id
//! length u64, tab vector length u64, menu (three bools), prefs (two u8
//! codes then three bools), overlay u8.

use serde::{Deserialize, Serialize};

use crate::theme::Theme;

/// Overlay codes. The chrome renders exactly one at a time.
pub const OVERLAY_NONE: u8 = 0;
pub const OVERLAY_TABS: u8 = 1;
pub const OVERLAY_SETTINGS: u8 = 2;
pub const OVERLAY_DEVTOOLS: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabStripItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuState {
    /// The three-dot menu only lists split view when the flag is on.
    pub split_visible: bool,
    pub split_active: bool,
    /// The update check entry only appears when the flag is on.
    pub updates_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefsState {
    /// ThemePreference code.
    pub theme_code: u8,
    /// SearchEngineId code.
    pub engine_code: u8,
    pub split_screen: bool,
    pub ad_blocker: bool,
    pub auto_update: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSnapshot {
    pub revision: u64,
    pub dark: bool,
    pub theme: Theme,
    /// The omnibox display text; empty for a blank tab.
    pub omnibox: String,
    pub can_back: bool,
    pub can_forward: bool,
    pub tab_count: u32,
    pub active_tab_id: String,
    pub tabs: Vec<TabStripItem>,
    pub menu: MenuState,
    pub prefs: PrefsState,
    pub overlay: u8,
}

/// One console entry of the developer tools feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleItem {
    pub level: String,
    pub message: String,
    pub at_ms: u64,
}

/// One network entry of the developer tools feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkItem {
    pub method: String,
    pub url: String,
    pub status: u32,
    pub duration_ms: u64,
    pub at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DevToolsSnapshot {
    pub console: Vec<ConsoleItem>,
    pub network: Vec<NetworkItem>,
}

pub fn encode_snapshot(snapshot: &UiSnapshot) -> Vec<u8> {
    bincode::serialize(snapshot).expect("snapshot encoding is infallible for these types")
}

pub fn decode_snapshot(bytes: &[u8]) -> Option<UiSnapshot> {
    bincode::deserialize(bytes).ok()
}

pub fn encode_devtools(snapshot: &DevToolsSnapshot) -> Vec<u8> {
    bincode::serialize(snapshot).expect("devtools encoding is infallible for these types")
}

pub fn decode_devtools(bytes: &[u8]) -> Option<DevToolsSnapshot> {
    bincode::deserialize(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemePreference;

    fn empty_snapshot() -> UiSnapshot {
        UiSnapshot {
            revision: 7,
            dark: false,
            theme: *Theme::resolve(ThemePreference::Light, false),
            omnibox: String::new(),
            can_back: false,
            can_forward: false,
            tab_count: 0,
            active_tab_id: String::new(),
            tabs: Vec::new(),
            menu: MenuState {
                split_visible: false,
                split_active: false,
                updates_visible: false,
            },
            prefs: PrefsState {
                theme_code: 0,
                engine_code: 0,
                split_screen: false,
                ad_blocker: false,
                auto_update: false,
            },
            overlay: OVERLAY_NONE,
        }
    }

    #[test]
    fn golden_layout() {
        let bytes = encode_snapshot(&empty_snapshot());
        // 8 revision + 1 dark + 40 theme + 8 omnibox length + 1 + 1 + 4
        // + 8 + 8 + 3 menu + 5 prefs + 1 overlay = 88.
        assert_eq!(bytes.len(), 88);
        assert_eq!(bytes[0], 7, "revision u64 little-endian");
        assert_eq!(bytes[8], 0, "dark bool");
        assert_eq!(&bytes[9..13], &[0xFF, 0xFF, 0xFF, 0xFF], "surface color");
        assert_eq!(
            &bytes[33..37],
            &[0xA6, 0x5A, 0x3C, 0xFF],
            "primary color, 0xFF3C5AA6 little-endian"
        );
        assert!(bytes[49..87].iter().all(|&byte| byte == 0));
        assert_eq!(bytes[87], OVERLAY_NONE);
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let mut snapshot = empty_snapshot();
        snapshot.revision = 42;
        snapshot.omnibox = "https://example.com/page".to_string();
        snapshot.can_back = true;
        snapshot.tab_count = 3;
        snapshot.active_tab_id = "tab-session-2".to_string();
        snapshot.tabs = vec![
            TabStripItem {
                id: "tab-session-1".to_string(),
                title: "First".to_string(),
                url: "https://one.example/".to_string(),
                active: false,
            },
            TabStripItem {
                id: "tab-session-2".to_string(),
                title: "Second".to_string(),
                url: "https://two.example/".to_string(),
                active: true,
            },
            TabStripItem {
                id: "tab-session-3".to_string(),
                title: "Third".to_string(),
                url: "https://three.example/".to_string(),
                active: false,
            },
        ];
        snapshot.menu = MenuState {
            split_visible: true,
            split_active: false,
            updates_visible: true,
        };
        snapshot.prefs = PrefsState {
            theme_code: 2,
            engine_code: 1,
            split_screen: true,
            ad_blocker: true,
            auto_update: true,
        };
        snapshot.overlay = OVERLAY_TABS;
        let decoded = decode_snapshot(&encode_snapshot(&snapshot)).expect("round trip");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn string_lengths_are_u64_and_carry_utf8() {
        let mut snapshot = empty_snapshot();
        snapshot.omnibox = "https://x.io".to_string();
        let bytes = encode_snapshot(&snapshot);
        let mut length = 0u64;
        for &byte in &bytes[49..57] {
            length = (length >> 8) | ((byte as u64) << 56);
        }
        assert_eq!(length, 12, "omnibox string length is a u64 at 49..57");
        assert_eq!(&bytes[57..69], b"https://x.io");
    }

    #[test]
    fn garbage_bytes_decode_to_none() {
        let bytes = encode_snapshot(&empty_snapshot());
        let mut corrupt = bytes.clone();
        corrupt.truncate(40);
        assert_eq!(decode_snapshot(&corrupt), None);
        assert_eq!(decode_snapshot(&[0u8; 4]), None);
    }

    #[test]
    fn devtools_snapshots_round_trip() {
        let snapshot = DevToolsSnapshot {
            console: vec![ConsoleItem {
                level: "error".to_string(),
                message: "Failed to load: net::ERR_NAME_NOT_RESOLVED".to_string(),
                at_ms: 1_700_000_000_000,
            }],
            network: vec![NetworkItem {
                method: "GET".to_string(),
                url: "https://example.com/x".to_string(),
                status: 200,
                duration_ms: 42,
                at_ms: 1_700_000_000_000,
            }],
        };
        assert_eq!(decode_devtools(&encode_devtools(&snapshot)), Some(snapshot));
    }
}
