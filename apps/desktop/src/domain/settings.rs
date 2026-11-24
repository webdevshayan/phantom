use serde::{Deserialize, Serialize};

use crate::domain::search::SearchEngine;

/// Bumped whenever persisted settings change shape. A blob written by a
/// newer build is ignored (defaults apply) rather than half-interpreted.
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

/// Appearance of the chrome. `System` follows the OS color scheme reported
/// by the platform at launch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

/// Experimental features, each gated behind a switch in the settings window.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureFlag {
    EnableTiling,
    EnableDragTabs,
    EnableAutoUpdater,
}

impl FeatureFlag {
    pub const ALL: &'static [FeatureFlag] = &[
        FeatureFlag::EnableTiling,
        FeatureFlag::EnableDragTabs,
        FeatureFlag::EnableAutoUpdater,
    ];

    pub fn id(self) -> &'static str {
        match self {
            FeatureFlag::EnableTiling => "enable_tiling",
            FeatureFlag::EnableDragTabs => "enable_drag_tabs",
            FeatureFlag::EnableAutoUpdater => "enable_auto_updater",
        }
    }

    pub fn from_id(raw: &str) -> Option<Self> {
        FeatureFlag::ALL.iter().copied().find(|flag| flag.id() == raw)
    }

    pub fn description(self) -> &'static str {
        match self {
            FeatureFlag::EnableTiling => "Split the window into panes (horizontal / vertical tiling)",
            FeatureFlag::EnableDragTabs => "Drag a tab out of the strip to open it in a new window",
            FeatureFlag::EnableAutoUpdater => "Check for Phantom updates in the background",
        }
    }

    /// Everything ships disabled until it has survived review; the updater
    /// flag additionally has no implementation behind it yet.
    pub fn default_value(self) -> bool {
        false
    }
}

/// Error for actions whose feature flag is off. Fails closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
#[error("feature '{0}' is disabled in settings")]
pub struct FeatureDisabledError(pub &'static str);

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct FeatureFlags {
    #[serde(default)]
    pub enable_tiling: bool,
    #[serde(default)]
    pub enable_drag_tabs: bool,
    #[serde(default)]
    pub enable_auto_updater: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enable_tiling: FeatureFlag::EnableTiling.default_value(),
            enable_drag_tabs: FeatureFlag::EnableDragTabs.default_value(),
            enable_auto_updater: FeatureFlag::EnableAutoUpdater.default_value(),
        }
    }
}

impl FeatureFlags {
    pub fn get(self, flag: FeatureFlag) -> bool {
        match flag {
            FeatureFlag::EnableTiling => self.enable_tiling,
            FeatureFlag::EnableDragTabs => self.enable_drag_tabs,
            FeatureFlag::EnableAutoUpdater => self.enable_auto_updater,
        }
    }

    pub fn set(&mut self, flag: FeatureFlag, enabled: bool) {
        match flag {
            FeatureFlag::EnableTiling => self.enable_tiling = enabled,
            FeatureFlag::EnableDragTabs => self.enable_drag_tabs = enabled,
            FeatureFlag::EnableAutoUpdater => self.enable_auto_updater = enabled,
        }
    }

    /// Guard used by session handling: a gated action errors out when the
    /// flag is off, so UI mistakes can never bypass settings.
    pub fn require(self, flag: FeatureFlag) -> Result<(), FeatureDisabledError> {
        if self.get(flag) {
            Ok(())
        } else {
            Err(FeatureDisabledError(flag.id()))
        }
    }
}

fn default_theme() -> Theme {
    Theme::System
}

fn default_search_engine() -> SearchEngine {
    SearchEngine::DuckDuckGo
}

/// Everything persisted under the `settings` key. Fields added in later
/// versions deserialize to defaults on older blobs. Field defaults are
/// pinned via functions so they cannot drift from `Settings::default()`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    #[serde(default = "default_theme")]
    pub theme: Theme,
    #[serde(default = "default_search_engine")]
    pub search_engine: SearchEngine,
    #[serde(default)]
    pub flags: FeatureFlags,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            theme: Theme::System,
            search_engine: SearchEngine::DuckDuckGo,
            flags: FeatureFlags::default(),
        }
    }
}

impl Settings {
    /// Applies a stored blob over the defaults. Blobs stamped with a newer
    /// schema version fall back to defaults instead of being guessed at.
    pub fn from_stored(json: &str) -> Result<Settings, serde_json::Error> {
        let stored: Settings = serde_json::from_str(json)?;
        if stored.schema_version > SETTINGS_SCHEMA_VERSION {
            return Ok(Settings::default());
        }
        Ok(Settings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            ..stored
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_a_private_starting_point() {
        let settings = Settings::default();
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.search_engine, SearchEngine::DuckDuckGo);
        assert!(!settings.flags.enable_tiling);
        assert!(!settings.flags.enable_drag_tabs);
        assert!(!settings.flags.enable_auto_updater);
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
    }

    #[test]
    fn flag_ids_roundtrip() {
        for flag in FeatureFlag::ALL {
            assert_eq!(FeatureFlag::from_id(flag.id()), Some(*flag));
        }
        assert_eq!(FeatureFlag::from_id("enable_tiling"), Some(FeatureFlag::EnableTiling));
        assert_eq!(FeatureFlag::from_id("enable_flying"), None);
    }

    #[test]
    fn require_fails_closed_until_enabled() {
        let mut flags = FeatureFlags::default();
        assert_eq!(
            flags.require(FeatureFlag::EnableTiling),
            Err(FeatureDisabledError("enable_tiling"))
        );
        flags.set(FeatureFlag::EnableTiling, true);
        assert_eq!(flags.require(FeatureFlag::EnableTiling), Ok(()));
        assert!(flags.get(FeatureFlag::EnableTiling));
        flags.set(FeatureFlag::EnableTiling, false);
        assert!(!flags.get(FeatureFlag::EnableTiling));
    }

    #[test]
    fn older_blob_deserializes_with_defaults_for_new_fields() {
        let blob = "{\"schema_version\":1}";
        let settings = Settings::from_stored(blob).unwrap();
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.search_engine, SearchEngine::DuckDuckGo);
        assert!(!settings.flags.enable_tiling);
    }

    #[test]
    fn newer_blob_falls_back_to_defaults() {
        let blob = format!(
            "{{\"schema_version\":{},\"theme\":\"dark\",\"search_engine\":\"google\",\"flags\":{{\"enable_tiling\":true}}}}",
            SETTINGS_SCHEMA_VERSION + 7
        );
        let settings = Settings::from_stored(&blob).unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn settings_serialize_with_snake_case_values() {
        let mut settings = Settings::default();
        settings.theme = Theme::Dark;
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"theme\":\"dark\""), "{json}");
        let restored = Settings::from_stored(&json).unwrap();
        assert_eq!(restored, settings);
    }
}
