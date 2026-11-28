//! One open tab, and the full set of open tabs for one Space, including
//! the navigation history that lets the stack be restored after a
//! restart. (Port of the old tab entry and tab stack models.)

use serde::{Deserialize, Serialize};

/// Placeholder location of a tab that has not navigated yet.
pub const BLANK_URL: &str = "about:blank";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabEntry {
    pub id: String,
    pub title: String,
    pub history: Vec<String>,
    pub history_index: i32,
}

impl TabEntry {
    pub fn blank(id: String, url: &str) -> Self {
        Self {
            id,
            title: "New tab".to_string(),
            history: vec![url.to_string()],
            history_index: 0,
        }
    }

    pub fn url(&self) -> &str {
        if self.history.is_empty() {
            return BLANK_URL;
        }
        let index = self.history_index.clamp(0, self.history.len() as i32 - 1) as usize;
        &self.history[index]
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index >= 0 && (self.history_index as usize) + 1 < self.history.len()
    }

    /// Moves to `next_url`, trimming any forward entries so the back
    /// stack never forks.
    pub fn navigate_to(&self, next_url: &str) -> Self {
        if next_url == self.url() {
            return self.clone();
        }
        if self.history.is_empty() {
            return Self {
                id: self.id.clone(),
                title: self.title.clone(),
                history: vec![next_url.to_string()],
                history_index: 0,
            };
        }
        let keep = self.history_index.clamp(0, self.history.len() as i32 - 1) as usize;
        let mut trimmed = self.history[..=keep].to_vec();
        trimmed.push(next_url.to_string());
        let history_index = (trimmed.len() - 1) as i32;
        Self {
            id: self.id.clone(),
            title: self.title.clone(),
            history: trimmed,
            history_index,
        }
    }

    pub fn go_back(&self) -> Self {
        if self.can_go_back() {
            self.at(self.history_index - 1)
        } else {
            self.clone()
        }
    }

    pub fn go_forward(&self) -> Self {
        if self.can_go_forward() {
            self.at(self.history_index + 1)
        } else {
            self.clone()
        }
    }

    pub fn with_title(&self, next_title: &str) -> Self {
        Self {
            id: self.id.clone(),
            title: next_title.to_string(),
            history: self.history.clone(),
            history_index: self.history_index,
        }
    }

    fn at(&self, index: i32) -> Self {
        Self {
            id: self.id.clone(),
            title: self.title.clone(),
            history: self.history.clone(),
            history_index: index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabStack {
    pub space_id: String,
    pub tabs: Vec<TabEntry>,
    pub active_tab_id: String,
}

impl TabStack {
    pub fn initial(space_id: &str) -> Self {
        let entry = TabEntry::blank(format!("tab-{space_id}-0"), BLANK_URL);
        Self {
            space_id: space_id.to_string(),
            tabs: vec![entry.clone()],
            active_tab_id: entry.id,
        }
    }

    /// The active tab, falling back to the first tab when the id does not
    /// resolve. The stack is never empty; the session keeps that
    /// invariant by replacing a fully closed stack with a fresh tab.
    pub fn active(&self) -> &TabEntry {
        self.tabs
            .iter()
            .find(|tab| tab.id == self.active_tab_id)
            .unwrap_or_else(|| &self.tabs[0])
    }

    pub fn by_id(&self, id: &str) -> Option<&TabEntry> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub fn with_tabs(&self, next_tabs: Vec<TabEntry>, next_active_id: Option<&str>) -> Self {
        Self {
            space_id: self.space_id.clone(),
            active_tab_id: next_active_id
                .map(str::to_string)
                .unwrap_or_else(|| self.active_tab_id.clone()),
            tabs: next_tabs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> TabEntry {
        TabEntry::blank("tab-1".to_string(), "https://a.example/")
    }

    #[test]
    fn blank_starts_at_the_given_location() {
        let tab = entry();
        assert_eq!(tab.url(), "https://a.example/");
        assert_eq!(tab.title, "New tab");
        assert!(!tab.can_go_back());
        assert!(!tab.can_go_forward());
    }

    #[test]
    fn navigation_trims_the_forward_stack() {
        let tab = entry()
            .navigate_to("https://b.example/")
            .navigate_to("https://c.example/");
        assert_eq!(tab.url(), "https://c.example/");
        let back = tab.go_back();
        assert_eq!(back.url(), "https://b.example/");
        let forward = back.go_forward();
        assert_eq!(forward.url(), "https://c.example/");
        let forked = back.navigate_to("https://d.example/");
        assert_eq!(forked.url(), "https://d.example/");
        assert!(!forked.can_go_forward(), "the fork must drop c.example");
        assert_eq!(forked.go_back().url(), "https://b.example/");
    }

    #[test]
    fn navigating_to_the_same_location_is_a_no_op() {
        let tab = entry().navigate_to("https://b.example/");
        let again = tab.navigate_to("https://b.example/");
        assert_eq!(again, tab);
    }

    #[test]
    fn bounds_are_clamped() {
        let tab = entry();
        assert_eq!(tab.go_back().url(), tab.url());
        assert_eq!(tab.go_forward().url(), tab.url());
    }

    #[test]
    fn stack_resolves_the_active_tab() {
        let stack = TabStack::initial("session");
        assert_eq!(stack.active().id, "tab-session-0");
        assert_eq!(stack.by_id("nope"), None);
        let renamed = stack.with_tabs(
            vec![stack.active().with_title("Docs")],
            Some("tab-session-0"),
        );
        assert_eq!(renamed.active().title, "Docs");
    }
}
