//! Draft text captured from a page's form fields. Password, hidden, file,
//! checkbox and radio inputs are never captured — the exclusion lives in
//! the injected collector and is pinned by tests — and the values only
//! ever live inside the Space's own data directory. (Port of the old form
//! snapshot model; the field map keeps DOM order.)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormSnapshot {
    pub page_url: String,
    pub fields: Vec<(String, String)>,
}

impl FormSnapshot {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_carry_page_and_fields() {
        let snapshot = FormSnapshot {
            page_url: "https://example.com/login".to_string(),
            fields: vec![
                ("user".to_string(), "ada".to_string()),
                ("note".to_string(), "hello".to_string()),
            ],
        };
        assert!(!snapshot.is_empty());
        assert_eq!(snapshot.fields[0].0, "user");
    }
}
