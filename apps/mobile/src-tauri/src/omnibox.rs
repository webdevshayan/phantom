//! Turns whatever the user typed into a loadable location. Inputs with an
//! http(s) scheme pass through untouched, host-like inputs get an https
//! scheme, and everything else becomes a query for the configured engine.
//! (Port of the old omnibox resolution.)

use crate::search::{build_query_url, SearchEngine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmniboxKind {
    Url,
    Search,
}

/// What the omnibox input resolved to: either a direct location or a
/// search query built with the configured engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmniboxTarget {
    pub kind: OmniboxKind,
    pub url: String,
    pub raw_input: String,
}

pub fn resolve_omnibox_input(raw_input: &str, engine: &SearchEngine) -> OmniboxTarget {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return OmniboxTarget {
            kind: OmniboxKind::Url,
            url: engine.home_url.to_string(),
            raw_input: trimmed.to_string(),
        };
    }
    if has_http_scheme(trimmed) {
        return OmniboxTarget {
            kind: OmniboxKind::Url,
            url: trimmed.to_string(),
            raw_input: trimmed.to_string(),
        };
    }
    if looks_like_host(trimmed) {
        return OmniboxTarget {
            kind: OmniboxKind::Url,
            url: format!("https://{trimmed}"),
            raw_input: trimmed.to_string(),
        };
    }
    OmniboxTarget {
        kind: OmniboxKind::Search,
        url: build_query_url(engine, trimmed),
        raw_input: trimmed.to_string(),
    }
}

/// True when a scheme terminator appears before any path separator and
/// the scheme is http or https. Case is ignored, matching the old
/// platform parser which lowercased schemes.
fn has_http_scheme(input: &str) -> bool {
    let colon = match input.find(':') {
        Some(index) => index,
        None => return false,
    };
    if let Some(slash) = input.find('/') {
        if slash < colon {
            return false;
        }
    }
    let scheme = &input[..colon];
    scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
}

fn looks_like_host(input: &str) -> bool {
    if input.contains(' ') {
        return false;
    }
    let host = input.split('/').next().unwrap_or("");
    if host.starts_with("localhost") {
        return true;
    }
    if !host.contains('.') {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.iter().any(|label| label.is_empty()) {
        return false;
    }
    let tld = labels[labels.len() - 1];
    tld.len() >= 2 && tld.bytes().all(|byte| byte.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{by_id, SearchEngineId};

    fn resolve(input: &str) -> OmniboxTarget {
        resolve_omnibox_input(input, by_id(SearchEngineId::Google))
    }

    #[test]
    fn empty_input_lands_on_the_engine_home() {
        let target = resolve("");
        assert_eq!(target.kind, OmniboxKind::Url);
        assert_eq!(target.url, "https://www.google.com");
        let target = resolve("   ");
        assert_eq!(target.url, "https://www.google.com");
    }

    #[test]
    fn scheme_inputs_pass_through() {
        for input in [
            "https://example.com",
            "http://example.com",
            "HTTPS://EXAMPLE.COM/PATH",
        ] {
            let target = resolve(input);
            assert_eq!(target.kind, OmniboxKind::Url, "{input}");
            assert_eq!(target.url, input);
        }
    }

    #[test]
    fn host_like_inputs_get_an_https_scheme() {
        for (input, expected) in [
            ("example.com", "https://example.com"),
            ("example.com/path", "https://example.com/path"),
            ("localhost", "https://localhost"),
            ("localhost:8080", "https://localhost:8080"),
            ("sub.site.io", "https://sub.site.io"),
        ] {
            let target = resolve(input);
            assert_eq!(target.kind, OmniboxKind::Url, "{input}");
            assert_eq!(target.url, expected);
        }
    }

    #[test]
    fn everything_else_becomes_a_search() {
        let target = resolve("how do browsers work");
        assert_eq!(target.kind, OmniboxKind::Search);
        assert_eq!(
            target.url,
            "https://www.google.com/search?q=how%20do%20browsers%20work"
        );
        let target = resolve("about:blank");
        assert_eq!(target.kind, OmniboxKind::Search);
    }

    #[test]
    fn host_heuristic_rejects_bare_words_and_bad_tlds() {
        assert_eq!(resolve("intranet").kind, OmniboxKind::Search);
        assert_eq!(resolve("a.c").kind, OmniboxKind::Search);
        assert_eq!(resolve("site.c0m").kind, OmniboxKind::Search);
        assert_eq!(resolve("two words.com").kind, OmniboxKind::Search);
    }
}
