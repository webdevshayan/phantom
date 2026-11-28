//! A Space is an isolated container for browsing data. Every Space owns a
//! dedicated directory on disk and nothing is ever shared between Spaces:
//! the tab stack, preferences and form drafts all live inside the Space
//! that produced them. (Port of the old domain space model.)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub label: String,
}

impl Space {
    /// The Space the mobile shell runs in for this session.
    pub fn session() -> Self {
        Self {
            id: "session".to_string(),
            label: "Session".to_string(),
        }
    }

    /// Path segment used for this Space's data directory. It is a plain,
    /// sanitized string so directory boundaries cannot be escaped.
    pub fn directory_segment(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_space_is_fixed() {
        let space = Space::session();
        assert_eq!(space.id, "session");
        assert_eq!(space.label, "Session");
        assert_eq!(space.directory_segment(), "session");
    }
}
