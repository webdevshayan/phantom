//! The theme model. Every color the injected chrome renders with is
//! defined here, resolved once in the core, and shipped to the UI inside
//! the binary snapshot. The UI never picks colors on its own.

use serde::{Deserialize, Serialize};

/// One opaque color, packed 0xRRGGBBAA.
pub type Rgba = u32;

/// The three theme options the settings page offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreference {
    Light,
    Dark,
    FollowDevice,
}

impl ThemePreference {
    /// Wire code used by the snapshot and the command surface.
    pub const fn code(self) -> u8 {
        match self {
            Self::Light => 0,
            Self::Dark => 1,
            Self::FollowDevice => 2,
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Light),
            1 => Some(Self::Dark),
            2 => Some(Self::FollowDevice),
            _ => None,
        }
    }

    /// Stable setting identifier, matching the persisted file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::FollowDevice => "followDevice",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "followDevice" => Some(Self::FollowDevice),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::FollowDevice => "Follow device",
        }
    }
}

/// Every fill and stroke the chrome paints. Flat, opaque surfaces only:
/// no gradients, no translucency, no shadows in the palette. The two
/// accent anchors are carried over from the previous seed colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// Chrome and page backdrop.
    pub surface: Rgba,
    /// Omnibox pill, menus, list rows.
    pub surface_container: Rgba,
    /// Raised rows, the active tab strip item.
    pub surface_container_high: Rgba,
    /// Primary text.
    pub on_surface: Rgba,
    /// Secondary text, hints, resting icons.
    pub on_surface_variant: Rgba,
    /// Borders and the tab count ring.
    pub outline: Rgba,
    /// Accent: active states, selection, links.
    pub primary: Rgba,
    /// Text and glyphs on primary fills.
    pub on_primary: Rgba,
    /// Destructive actions and load errors.
    pub danger: Rgba,
    /// Hairline separators, including the split view divider.
    pub divider: Rgba,
}

impl Theme {
    pub const LIGHT: Theme = Theme {
        surface: 0xFFFF_FFFF,
        surface_container: 0xFFF1_F3F7,
        surface_container_high: 0xFFE6_EAF1,
        on_surface: 0xFF1A_1E27,
        on_surface_variant: 0xFF59_606F,
        outline: 0xFFC4_CAD5,
        primary: 0xFF3C_5AA6,
        on_primary: 0xFFFF_FFFF,
        danger: 0xFFB3_261E,
        divider: 0x3388_8888,
    };

    pub const DARK: Theme = Theme {
        surface: 0xFF14_161B,
        surface_container: 0xFF1E_222A,
        surface_container_high: 0xFF28_2D38,
        on_surface: 0xFFE4_E7EE,
        on_surface_variant: 0xFF9A_A3B2,
        outline: 0xFF45_4C59,
        primary: 0xFF8A_B4F8,
        on_primary: 0xFF0B_1220,
        danger: 0xFFF2_B8B5,
        divider: 0x3388_8888,
    };

    /// Maps the stored preference onto a concrete palette. The
    /// follow-device option reads the platform brightness directly.
    pub fn resolve(preference: ThemePreference, platform_dark: bool) -> &'static Theme {
        match preference {
            ThemePreference::Light => &Self::LIGHT,
            ThemePreference::Dark => &Self::DARK,
            ThemePreference::FollowDevice => {
                if platform_dark {
                    &Self::DARK
                } else {
                    &Self::LIGHT
                }
            }
        }
    }

    pub fn is_dark(preference: ThemePreference, platform_dark: bool) -> bool {
        matches!(Self::resolve(preference, platform_dark), &Self::DARK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_maps_each_preference() {
        // Compare by value: associated consts are inlined at every use
        // site, so pointer equality is meaningless for them.
        assert_eq!(*Theme::resolve(ThemePreference::Light, true), Theme::LIGHT);
        assert_eq!(*Theme::resolve(ThemePreference::Dark, false), Theme::DARK);
        assert_eq!(
            *Theme::resolve(ThemePreference::FollowDevice, false),
            Theme::LIGHT
        );
        assert_eq!(
            *Theme::resolve(ThemePreference::FollowDevice, true),
            Theme::DARK
        );
        assert!(Theme::is_dark(ThemePreference::Light, true) == false);
        assert!(Theme::is_dark(ThemePreference::Dark, true));
        assert!(Theme::is_dark(ThemePreference::FollowDevice, true));
        assert!(!Theme::is_dark(ThemePreference::FollowDevice, false));
    }

    #[test]
    fn preference_keys_round_trip() {
        for preference in [
            ThemePreference::Light,
            ThemePreference::Dark,
            ThemePreference::FollowDevice,
        ] {
            assert_eq!(ThemePreference::from_key(preference.key()), Some(preference));
            assert_eq!(
                ThemePreference::from_code(preference.code()),
                Some(preference)
            );
        }
        assert_eq!(ThemePreference::from_key("nonsense"), None);
        assert_eq!(ThemePreference::from_code(9), None);
    }

    #[test]
    fn seed_anchors_survive() {
        assert_eq!(Theme::LIGHT.primary, 0xFF3C_5AA6);
        assert_eq!(Theme::DARK.primary, 0xFF8A_B4F8);
    }
}
