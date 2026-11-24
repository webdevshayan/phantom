use url::Url;

use crate::domain::command::validate_navigation;
use crate::domain::search::SearchEngine;

/// What omnibox input means once classified.
#[derive(Clone, Debug, PartialEq)]
pub enum OmniboxInput {
    /// A direct navigation, already normalized with a scheme.
    Url(Url),
    /// A web search for the given text on the active engine.
    Search(String),
}

/// Classifies omnibox input the way a mainstream browser does:
/// - explicit `http` / `https` / `file` / `phantom` urls pass through as-is
/// - scheme-less input that looks like a host (dots, localhost, ipv4, port)
///   is promoted to an http url
/// - everything else, including dangerous schemes, becomes a search
pub fn classify(input: &str) -> OmniboxInput {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return OmniboxInput::Search(String::new());
    }

    // `Url::parse` accepts dotted "schemes" like `example.com:8080`, so a
    // non-navigable parse result can still be a host reference — fall
    // through to host promotion before settling on a search.
    if let Ok(url) = Url::parse(trimmed) {
        if is_navigable(&url) {
            return OmniboxInput::Url(url);
        }
    }

    if looks_like_host(trimmed) {
        if let Ok(url) = Url::parse(&format!("http://{trimmed}")) {
            if is_navigable(&url) {
                return OmniboxInput::Url(url);
            }
        }
    }

    OmniboxInput::Search(trimmed.to_string())
}

/// Resolves omnibox text to a navigation url using the active engine.
pub fn resolve(input: &str, engine: SearchEngine) -> Url {
    match classify(input) {
        OmniboxInput::Url(url) => url,
        OmniboxInput::Search(query) => engine.search_url(&query),
    }
}

fn is_navigable(url: &Url) -> bool {
    // `javascript:` and friends parse fine as urls but must never pass
    validate_navigation(url).is_ok()
}

/// `example.com`, `localhost:3000`, `127.0.0.1`, `docs.example.org/book`
fn looks_like_host(input: &str) -> bool {
    if input.contains(char::is_whitespace) {
        return false;
    }
    let authority = input.split(['/', '?', '#']).next().unwrap_or(input);
    let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
    if host.is_empty() {
        return false;
    }
    host == "localhost"
        || host.parse::<std::net::Ipv4Addr>().is_ok()
        || host.contains('.')
}

/// Text-editing state behind the omnibox widget. Kept in the domain layer so
/// caret math is unit-testable without a toolkit.
#[derive(Clone, Debug, Default)]
pub struct OmniboxBuffer {
    text: String,
    cursor: usize,
    focused: bool,
}

impl OmniboxBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads the active tab url for display and selects nothing (caret at
    /// the end), mirroring omnibox focus behavior.
    pub fn show_url(&mut self, url: Option<&Url>) {
        self.text = url.map(|u| u.to_string()).unwrap_or_default();
        self.cursor = self.text.len();
        self.focused = false;
    }

    pub fn focus(&mut self, select_all: bool) {
        self.focused = true;
        if select_all {
            self.select_all();
        }
    }

    pub fn blur(&mut self) {
        self.focused = false;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Caret position as a byte offset, always on a char boundary.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.text.len();
    }

    /// Inserts text at the caret (typed characters, paste).
    pub fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.clamp_cursor();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let previous = self.previous_boundary(self.cursor);
            self.text.replace_range(previous..self.cursor, "");
            self.cursor = previous;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            let next = self.next_boundary(self.cursor);
            self.text.replace_range(self.cursor..next, "");
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.previous_boundary(self.cursor);
    }

    pub fn move_right(&mut self) {
        self.cursor = self.next_boundary(self.cursor);
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    pub fn select_all(&mut self) {
        self.cursor = self.text.len();
    }

    fn clamp_cursor(&mut self) {
        while !self.text.is_char_boundary(self.cursor) && self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn previous_boundary(&self, from: usize) -> usize {
        self.text[..from]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next_boundary(&self, from: usize) -> usize {
        self.text[from..]
            .chars()
            .next()
            .map_or(from, |c| from + c.len_utf8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_explicit_urls() {
        assert_eq!(
            classify("https://example.com/a?b=c"),
            OmniboxInput::Url(Url::parse("https://example.com/a?b=c").unwrap())
        );
        assert_eq!(
            classify("phantom://newtab"),
            OmniboxInput::Url(Url::parse("phantom://newtab").unwrap())
        );
        assert_eq!(
            classify("file:///etc/hosts"),
            OmniboxInput::Url(Url::parse("file:///etc/hosts").unwrap())
        );
    }

    #[test]
    fn promotes_host_like_input_to_http() {
        for input in ["example.com", "example.com:8080/path", "localhost", "127.0.0.1"] {
            let expected = Url::parse(&format!("http://{input}")).unwrap();
            assert_eq!(classify(input), OmniboxInput::Url(expected), "{input}");
        }
        assert_eq!(
            classify("EXAMPLE.COM"),
            OmniboxInput::Url(Url::parse("http://EXAMPLE.COM").unwrap())
        );
    }

    #[test]
    fn sends_text_and_dangerous_schemes_to_search() {
        assert_eq!(classify("rust vs go benchmark"), OmniboxInput::Search("rust vs go benchmark".into()));
        assert_eq!(classify(""), OmniboxInput::Search(String::new()));
        assert_eq!(classify("   "), OmniboxInput::Search(String::new()));
        assert_eq!(classify("1 + 1"), OmniboxInput::Search("1 + 1".into()));
        // typed javascript: is refused as navigation and treated as text
        assert!(matches!(classify("javascript:alert(1)"), OmniboxInput::Search(_)));
        assert!(matches!(classify("data:text/html,hi"), OmniboxInput::Search(_)));
        assert!(matches!(classify("blob:https://x"), OmniboxInput::Search(_)));
    }

    #[test]
    fn resolve_builds_engine_searches() {
        let url = resolve("rust async book", SearchEngine::Brave);
        assert_eq!(url.host_str(), Some("search.brave.com"));
        let direct = resolve("example.com", SearchEngine::Google);
        assert_eq!(direct.host_str(), Some("example.com"));
    }

    #[test]
    fn buffer_typing_and_backspace() {
        let mut buffer = OmniboxBuffer::new();
        buffer.insert("phantom://new");
        assert_eq!(buffer.text(), "phantom://new");
        buffer.backspace();
        assert_eq!(buffer.text(), "phantom://ne");
        buffer.move_home();
        buffer.insert("x");
        assert_eq!(buffer.text(), "xphantom://ne");
        buffer.move_end();
        buffer.delete();
        assert_eq!(buffer.text(), "xphantom://ne");
    }

    #[test]
    fn buffer_handles_multibyte_boundaries() {
        let mut buffer = OmniboxBuffer::new();
        buffer.insert("中文en");
        assert_eq!(buffer.cursor(), "中文en".len());
        buffer.move_left();
        buffer.move_left();
        assert_eq!(buffer.cursor(), "中文".len()); // caret between 文 and e
        buffer.backspace(); // removes 文
        assert_eq!(buffer.text(), "中en");
        buffer.move_right();
        buffer.move_right();
        buffer.move_right(); // clamps at the end
        assert_eq!(buffer.cursor(), buffer.text().len());
        buffer.backspace(); // removes n
        assert_eq!(buffer.text(), "中e");
    }

    #[test]
    fn show_url_loads_and_focus_selects() {
        let mut buffer = OmniboxBuffer::new();
        let url = Url::parse("https://example.org/").unwrap();
        buffer.show_url(Some(&url));
        assert_eq!(buffer.text(), "https://example.org/");
        assert!(!buffer.is_focused());
        buffer.focus(true);
        assert!(buffer.is_focused());
        buffer.blur();
        assert!(!buffer.is_focused());
    }
}
