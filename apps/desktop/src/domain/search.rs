use serde::{Deserialize, Serialize};
use url::Url;

/// Search engines the omnibox can use. Templates are https-only literals;
/// the query value is percent-encoded before substitution.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngine {
    #[default]
    DuckDuckGo,
    Google,
    Bing,
    Brave,
    Startpage,
}

impl SearchEngine {
    pub const ALL: &'static [SearchEngine] = &[
        SearchEngine::DuckDuckGo,
        SearchEngine::Google,
        SearchEngine::Bing,
        SearchEngine::Brave,
        SearchEngine::Startpage,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchEngine::DuckDuckGo => "DuckDuckGo",
            SearchEngine::Google => "Google",
            SearchEngine::Bing => "Bing",
            SearchEngine::Brave => "Brave",
            SearchEngine::Startpage => "Startpage",
        }
    }

    /// Template carrying a `{query}` placeholder.
    pub fn query_template(self) -> &'static str {
        match self {
            SearchEngine::DuckDuckGo => "https://duckduckgo.com/?q={query}",
            SearchEngine::Google => "https://www.google.com/search?q={query}",
            SearchEngine::Bing => "https://www.bing.com/search?q={query}",
            SearchEngine::Brave => "https://search.brave.com/search?q={query}",
            SearchEngine::Startpage => "https://www.startpage.com/sp/search?query={query}",
        }
    }

    /// Builds the search url for a query, percent-encoding the value.
    pub fn search_url(self, query: &str) -> Url {
        let raw = self.query_template().replace("{query}", &encode_query_value(query));
        Url::parse(&raw).expect("search templates are static and valid")
    }
}

/// Percent-encodes a query value the way form submission does, which every
/// engine accepts for its query parameter.
fn encode_query_value(value: &str) -> String {
    let pair = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", value)
        .finish();
    pair.strip_prefix("q=").unwrap_or(pair.as_str()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_has_label_and_https_template() {
        for engine in SearchEngine::ALL {
            assert!(!engine.label().is_empty());
            let template = engine.query_template();
            assert!(template.starts_with("https://"), "{template}");
            assert!(template.contains("{query}"), "{template}");
        }
    }

    #[test]
    fn search_url_encodes_spaces_and_unicode() {
        let url = SearchEngine::DuckDuckGo.search_url("rust lang 中文");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("duckduckgo.com"));
        let query = url.query().expect("search url carries a query");
        assert!(query.contains("rust+lang"), "{query}");
        assert!(!query.contains(' '), "{query}");
    }

    #[test]
    fn search_url_escapes_special_characters() {
        let url = SearchEngine::Google.search_url("a&b=c d");
        let query = url.query().unwrap();
        assert!(query.starts_with("q=a%26b%3Dc+d"), "{query}");
    }

    #[test]
    fn serde_uses_snake_case_names() {
        assert_eq!(
            serde_json::to_string(&SearchEngine::DuckDuckGo).unwrap(),
            "\"duckduckgo\""
        );
        let engine: SearchEngine = serde_json::from_str("\"brave\"").unwrap();
        assert_eq!(engine, SearchEngine::Brave);
    }
}
