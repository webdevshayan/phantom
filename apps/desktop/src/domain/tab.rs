use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::space::SpaceId;

/// Stable identifier of a tab within a running session. Tab ids are persisted
/// in the session snapshot and never reused after a restore.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct TabId(pub u64);

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Load status as reported back by the webview host.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabStatus {
    #[default]
    New,
    Loading,
    Loaded,
    Failed,
}

/// One tab. A tab always belongs to exactly one space; the webview data for
/// that space is isolated on disk, so tabs are never moved across spaces.
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: TabId,
    pub space: SpaceId,
    pub title: String,
    pub url: Option<url::Url>,
    pub status: TabStatus,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// True once a native webview surface exists for this tab (the session
    /// tracks it to emit attach effects lazily). Not persisted.
    pub surfaced: bool,
}

/// Sentinel title of a tab that has not loaded anything yet.
pub const NEW_TAB_TITLE: &str = "New Tab";

impl Tab {
    pub fn new(id: TabId, space: SpaceId) -> Self {
        Self {
            id,
            space,
            title: String::from(NEW_TAB_TITLE),
            url: None,
            status: TabStatus::New,
            can_go_back: false,
            can_go_forward: false,
            surfaced: false,
        }
    }

    /// Domain-side effect of starting a navigation. History membership flags
    /// stay untouched until the webview reports the committed navigation.
    pub fn begin_navigation(&mut self, url: url::Url) {
        self.url = Some(url);
        self.status = TabStatus::Loading;
    }

    /// Applies a validated `NavigationState` report coming over IPC.
    pub fn apply_state(
        &mut self,
        url: Option<url::Url>,
        title: &str,
        can_go_back: bool,
        can_go_forward: bool,
        is_loading: bool,
    ) {
        if let Some(url) = url {
            self.url = Some(url);
        }
        if !title.is_empty() {
            self.title = title.to_string();
        }
        self.can_go_back = can_go_back;
        self.can_go_forward = can_go_forward;
        self.status = if is_loading {
            TabStatus::Loading
        } else if self.url.is_some() {
            TabStatus::Loaded
        } else {
            TabStatus::New
        };
    }

    pub fn mark_failed(&mut self) {
        self.status = TabStatus::Failed;
    }

    /// Title shown in the tab strip; falls back to the host, then the
    /// sentinel label — never to an empty string. The "New Tab" sentinel is
    /// treated as no title so the host shows through on first paint.
    pub fn display_title(&self) -> &str {
        if !self.title.is_empty() && self.title != NEW_TAB_TITLE {
            return &self.title;
        }
        if let Some(url) = &self.url {
            if let Some(host) = url.host_str() {
                return host;
            }
        }
        NEW_TAB_TITLE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_tab() -> Tab {
        Tab::new(TabId(1), SpaceId::default_space())
    }

    #[test]
    fn new_tab_defaults() {
        let tab = fresh_tab();
        assert_eq!(tab.status, TabStatus::New);
        assert_eq!(tab.url, None);
        assert!(!tab.can_go_back);
        assert_eq!(tab.display_title(), "New Tab");
    }

    #[test]
    fn begin_navigation_sets_loading_and_url() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://phantom.app/").unwrap();
        tab.begin_navigation(url.clone());
        assert_eq!(tab.status, TabStatus::Loading);
        assert_eq!(tab.url, Some(url));
    }

    #[test]
    fn apply_state_commits_navigation_report() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://example.net/doc").unwrap();
        tab.begin_navigation(url.clone());
        tab.apply_state(Some(url.clone()), "Example", true, false, false);
        assert_eq!(tab.status, TabStatus::Loaded);
        assert_eq!(tab.title, "Example");
        assert_eq!(tab.display_title(), "Example");
        assert!(tab.can_go_back);
        assert!(!tab.can_go_forward);
    }

    #[test]
    fn apply_state_keeps_title_while_loading() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://example.net/doc").unwrap();
        tab.begin_navigation(url.clone());
        tab.apply_state(Some(url), "", false, false, true);
        assert_eq!(tab.status, TabStatus::Loading);
        assert_eq!(tab.display_title(), "example.net");
    }

    #[test]
    fn display_title_falls_back_to_host_then_label() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://example.org/a").unwrap();
        tab.begin_navigation(url);
        assert_eq!(tab.display_title(), "example.org");
        tab.mark_failed();
        assert_eq!(tab.status, TabStatus::Failed);
    }
}
