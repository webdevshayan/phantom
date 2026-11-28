//! Feature flags and the persisted preferences. Defaults keep every
//! experiment off until the user opts in. (Port of the old preferences
//! model; the flag keys match the previously persisted map exactly.)

use serde::{Deserialize, Serialize};

use crate::search::SearchEngineId;
use crate::theme::ThemePreference;

pub const SPLIT_SCREEN_KEY: &str = "enable_split_screen";
pub const AD_BLOCKER_KEY: &str = "enable_ad_blocker";
pub const AUTO_UPDATE_KEY: &str = "enable_auto_update";

/// The three experimental capabilities exposed on the settings page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_split_screen: bool,
    pub enable_ad_blocker: bool,
    pub enable_auto_update: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::ALL_OFF
    }
}

impl FeatureFlags {
    pub const ALL_OFF: Self = Self {
        enable_split_screen: false,
        enable_ad_blocker: false,
        enable_auto_update: false,
    };

    pub fn value_of(&self, key: &str) -> bool {
        match key {
            SPLIT_SCREEN_KEY => self.enable_split_screen,
            AD_BLOCKER_KEY => self.enable_ad_blocker,
            AUTO_UPDATE_KEY => self.enable_auto_update,
            _ => false,
        }
    }

    /// Unknown keys leave the flags untouched.
    pub fn with_value(&self, key: &str, value: bool) -> Self {
        match key {
            SPLIT_SCREEN_KEY => Self {
                enable_split_screen: value,
                ..*self
            },
            AD_BLOCKER_KEY => Self {
                enable_ad_blocker: value,
                ..*self
            },
            AUTO_UPDATE_KEY => Self {
                enable_auto_update: value,
                ..*self
            },
            _ => *self,
        }
    }
}

/// Everything the settings page persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: ThemePreference,
    pub search_engine: SearchEngineId,
    pub flags: FeatureFlags,
}

impl Preferences {
    pub const INITIAL: Self = Self {
        theme: ThemePreference::FollowDevice,
        search_engine: SearchEngineId::Google,
        flags: FeatureFlags::ALL_OFF,
    };

    pub fn with_theme(&self, theme: ThemePreference) -> Self {
        Self { theme, ..*self }
    }

    pub fn with_search_engine(&self, search_engine: SearchEngineId) -> Self {
        Self {
            search_engine,
            ..*self
        }
    }

    pub fn with_flags(&self, flags: FeatureFlags) -> Self {
        Self { flags, ..*self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_default_to_all_off() {
        assert_eq!(FeatureFlags::default(), FeatureFlags::ALL_OFF);
        assert!(!FeatureFlags::ALL_OFF.value_of(SPLIT_SCREEN_KEY));
        assert!(!FeatureFlags::ALL_OFF.value_of(AD_BLOCKER_KEY));
        assert!(!FeatureFlags::ALL_OFF.value_of(AUTO_UPDATE_KEY));
        assert!(!FeatureFlags::ALL_OFF.value_of("unknown"));
    }

    #[test]
    fn flag_values_update_by_key() {
        let flags = FeatureFlags::ALL_OFF.with_value(AD_BLOCKER_KEY, true);
        assert!(flags.value_of(AD_BLOCKER_KEY));
        assert!(!flags.value_of(SPLIT_SCREEN_KEY));
        assert_eq!(
            flags.with_value("unknown", true),
            flags,
            "unknown keys are ignored"
        );
    }

    #[test]
    fn preferences_start_with_the_documented_defaults() {
        assert_eq!(Preferences::INITIAL.theme, ThemePreference::FollowDevice);
        assert_eq!(
            Preferences::INITIAL.search_engine,
            SearchEngineId::Google
        );
        assert_eq!(Preferences::INITIAL.flags, FeatureFlags::ALL_OFF);
        let changed = Preferences::INITIAL
            .with_theme(ThemePreference::Dark)
            .with_search_engine(SearchEngineId::Bing);
        assert_eq!(changed.theme, ThemePreference::Dark);
        assert_eq!(changed.search_engine, SearchEngineId::Bing);
        assert_eq!(changed.flags, Preferences::INITIAL.flags);
    }
}
