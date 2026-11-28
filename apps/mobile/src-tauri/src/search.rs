//! The search engine catalog, query building and URL component encoding.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchEngineId {
    Google,
    DuckDuckGo,
    Bing,
    Startpage,
}

impl SearchEngineId {
    /// Wire code used by the snapshot and the command surface. The order
    /// matches the settings page catalog.
    pub const fn code(self) -> u8 {
        match self {
            Self::Google => 0,
            Self::DuckDuckGo => 1,
            Self::Bing => 2,
            Self::Startpage => 3,
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Google),
            1 => Some(Self::DuckDuckGo),
            2 => Some(Self::Bing),
            3 => Some(Self::Startpage),
            _ => None,
        }
    }

    /// Stable setting identifier, matching the persisted file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::DuckDuckGo => "duckDuckGo",
            Self::Bing => "bing",
            Self::Startpage => "startpage",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "google" => Some(Self::Google),
            "duckDuckGo" => Some(Self::DuckDuckGo),
            "bing" => Some(Self::Bing),
            "startpage" => Some(Self::Startpage),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchEngine {
    pub id: SearchEngineId,
    pub label: &'static str,
    pub query_template: &'static str,
    pub home_url: &'static str,
}

pub const CATALOG: [SearchEngine; 4] = [
    SearchEngine {
        id: SearchEngineId::Google,
        label: "Google",
        query_template: "https://www.google.com/search?q=%s",
        home_url: "https://www.google.com",
    },
    SearchEngine {
        id: SearchEngineId::DuckDuckGo,
        label: "DuckDuckGo",
        query_template: "https://duckduckgo.com/?q=%s",
        home_url: "https://duckduckgo.com",
    },
    SearchEngine {
        id: SearchEngineId::Bing,
        label: "Bing",
        query_template: "https://www.bing.com/search?q=%s",
        home_url: "https://www.bing.com",
    },
    SearchEngine {
        id: SearchEngineId::Startpage,
        label: "Startpage",
        query_template: "https://www.startpage.com/sp/search?query=%s",
        home_url: "https://www.startpage.com",
    },
];

pub fn by_id(id: SearchEngineId) -> &'static SearchEngine {
    CATALOG
        .iter()
        .find(|engine| engine.id == id)
        .expect("the catalog always covers every engine id")
}

/// Builds the query URL by substituting the first `%s` placeholder with
/// the percent-encoded query.
pub fn build_query_url(engine: &SearchEngine, query: &str) -> String {
    engine.query_template.replacen("%s", &encode_component(query), 1)
}

/// Percent-encodes a query component. RFC 3986 unreserved bytes pass
/// through; everything else becomes %XX.
pub fn encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0F) as usize] as char);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_consistent() {
        for (index, engine) in CATALOG.iter().enumerate() {
            assert_eq!(engine.id.code() as usize, index);
            assert_eq!(by_id(engine.id).label, engine.label);
            assert_eq!(SearchEngineId::from_key(engine.id.key()), Some(engine.id));
        }
    }

    #[test]
    fn query_urls_embed_the_encoded_term() {
        let url = build_query_url(by_id(SearchEngineId::Google), "rust std lib");
        assert_eq!(url, "https://www.google.com/search?q=rust%20std%20lib");
        let url = build_query_url(by_id(SearchEngineId::DuckDuckGo), "a&b=c");
        assert_eq!(url, "https://duckduckgo.com/?q=a%26b%3Dc");
    }

    #[test]
    fn encoding_keeps_unreserved_bytes() {
        assert_eq!(encode_component("aZ09-_.~"), "aZ09-_.~");
        assert_eq!(encode_component(" "), "%20");
        assert_eq!(encode_component("%"), "%25");
        assert_eq!(encode_component(""), "");
    }
}
