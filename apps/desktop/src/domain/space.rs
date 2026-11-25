use std::fmt;

use serde::{Deserialize, Serialize};

/// Identifier of a browsing space.
///
/// A space owns an isolated webview data directory (cookies, site storage,
/// caches). The directory name is derived directly from the id, so parsing is
/// strict: ASCII letters, digits and dashes only, which makes path traversal
/// impossible by construction.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(String);

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SpaceError {
    #[error("space id must be 1-32 characters of a-z, 0-9 and '-'")]
    Invalid,
}

/// The space every fresh window opens with.
pub const DEFAULT_SPACE: &str = "default";

impl SpaceId {
    pub fn default_space() -> Self {
        // guarded by unit tests; the literal is known-good
        Self::parse(DEFAULT_SPACE).expect("built-in space id is valid")
    }

    /// Parses and normalizes a space id coming from config, state or UI.
    pub fn parse(raw: &str) -> Result<Self, SpaceError> {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty() || normalized.len() > 32 {
            return Err(SpaceError::Invalid);
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(SpaceError::Invalid);
        }
        if normalized.starts_with('-') || normalized.ends_with('-') || normalized.contains("--") {
            return Err(SpaceError::Invalid);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpaceId({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normalizes_case_and_whitespace() {
        assert_eq!(SpaceId::parse("  Personal ").unwrap().as_str(), "personal");
        assert_eq!(SpaceId::parse("WORK").unwrap().as_str(), "work");
    }

    #[test]
    fn parse_accepts_valid_ids() {
        for id in ["default", "work", "a", "news-123", "0"] {
            assert!(SpaceId::parse(id).is_ok(), "{id} should parse");
        }
    }

    #[test]
    fn parse_rejects_traversal_and_bad_characters() {
        let long = "x".repeat(33);
        for bad in [
            "",
            " ",
            "..",
            "../etc",
            "a/b",
            "a\\b",
            "with space",
            "-lead",
            "trail-",
            "dou--ble",
            "under_score",
            "dōt",
            long.as_str(),
        ] {
            assert_eq!(
                SpaceId::parse(bad),
                Err(SpaceError::Invalid),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn serde_roundtrip_preserves_id() {
        let id = SpaceId::parse("work").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"work\"");
        assert_eq!(serde_json::from_str::<SpaceId>(&json).unwrap(), id);
    }
}
