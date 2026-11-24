//! Built-in `phantom://` pages. Static markup only: no remote assets, no
//! network scripts, and no user-controlled text reflected without escaping.

use url::Url;

use crate::domain::search::SearchEngine;
use crate::domain::settings::Theme;

pub const NEW_TAB_URL: &str = "phantom://newtab";
pub const BLANK_URL: &str = "phantom://blank";

/// True when a url targets a built-in page instead of the web.
pub fn is_internal(url: &Url) -> bool {
    url.scheme() == "phantom"
}

/// The new tab page url.
pub fn new_tab_url() -> Url {
    Url::parse(NEW_TAB_URL).expect("static internal url is valid")
}

/// Markup for a built-in page, or `None` for unknown `phantom` hosts (the
/// host falls back to the error page).
pub fn page_for(url: &Url, theme: Theme, engine: SearchEngine) -> Option<String> {
    if !is_internal(url) {
        return None;
    }
    match url.host_str() {
        Some("newtab") => Some(new_tab_page(engine, theme)),
        Some("blank") => Some(blank_page()),
        _ => None,
    }
}

/// Static load-failure page. `reason` is a fixed string, never remote text.
pub fn error_page(reason: &'static str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Phantom</title>\
<style>{STYLE}</style></head><body class=\"theme-system\"><main><h1>This page can't be displayed</h1>\
<p>{reason}</p></main></body></html>",
        STYLE = STYLE,
        reason = reason,
    )
}

pub fn new_tab_page(engine: SearchEngine, theme: Theme) -> String {
    let engine_name = html_escape(engine.label());
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>New Tab</title>\
<style>{STYLE}</style></head><body class=\"{theme_class}\"><main>\
<h1>Phantom</h1><p>Search with {engine_name} or type a url</p>\
</main></body></html>",
        STYLE = STYLE,
        theme_class = theme_css_class(theme),
        engine_name = engine_name,
    )
}

pub fn blank_page() -> String {
    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body></body></html>".to_string()
}

pub fn theme_css_class(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "theme-light",
        Theme::Dark => "theme-dark",
        Theme::System => "theme-system",
    }
}

/// Minimal HTML escaping for the few literals we interpolate.
pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const STYLE: &str = ":root { color-scheme: light dark; } \
* { margin: 0; box-sizing: border-box; } \
body { font-family: system-ui, sans-serif; min-height: 100vh; display: flex; align-items: center; justify-content: center; } \
main { text-align: center; padding: 2rem; } \
h1 { font-size: 2.2rem; letter-spacing: 0.08em; font-weight: 600; } \
p { margin-top: 0.75rem; opacity: 0.7; } \
body.theme-light { background: #ffffff; color: #202124; } \
body.theme-dark { background: #202124; color: #e8eaed; } \
@media (prefers-color-scheme: dark) { body.theme-system { background: #202124; color: #e8eaed; } } \
@media (prefers-color-scheme: light) { body.theme-system { background: #ffffff; color: #202124; } }";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_urls_route_to_pages() {
        let theme = Theme::Dark;
        let engine = SearchEngine::DuckDuckGo;
        let newtab = Url::parse(NEW_TAB_URL).unwrap();
        assert!(is_internal(&newtab));
        let page = page_for(&newtab, theme, engine).expect("newtab page");
        assert!(page.contains("DuckDuckGo"));
        assert!(page.contains("theme-dark"));

        let blank = Url::parse(BLANK_URL).unwrap();
        assert!(page_for(&blank, theme, engine).is_some());

        let unknown = Url::parse("phantom://settings").unwrap();
        assert_eq!(page_for(&unknown, theme, engine), None);
    }

    #[test]
    fn external_urls_never_route_here() {
        let external = Url::parse("https://example.com/phantom://newtab").unwrap();
        assert!(!is_internal(&external));
    }

    #[test]
    fn html_escape_neutralizes_markup() {
        assert_eq!(
            html_escape("<script>alert(\"x\")</script> & 'oops'"),
            "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;oops&#39;"
        );
    }

    #[test]
    fn error_pages_use_static_reasons_only() {
        let page = error_page("The address is invalid");
        assert!(page.contains("The address is invalid"));
        assert!(!page.contains("<script"));
    }

    #[test]
    fn new_tab_url_is_valid() {
        assert_eq!(new_tab_url().as_str(), NEW_TAB_URL);
    }
}
