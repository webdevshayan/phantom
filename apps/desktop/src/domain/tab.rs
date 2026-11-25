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
    /// Committed top-document entries, newest last. `history_position`
    /// marks the entry the webview currently shows; back and forward
    /// availability derive from it. Not persisted: a restored tab starts
    /// with a single entry, like a session restore in any browser.
    history: Vec<url::Url>,
    history_position: usize,
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
            history: Vec::new(),
            history_position: 0,
            surfaced: false,
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_position > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_position + 1 < self.history.len()
    }

    /// Chrome-side navigation (omnibox, menus, internal pages). A new url
    /// truncates the forward stack and appends, like typing an address;
    /// re-navigating the current entry (reload) leaves history alone.
    pub fn begin_navigation(&mut self, url: url::Url) {
        if self.history.get(self.history_position) == Some(&url) {
            self.url = Some(url);
            self.status = TabStatus::Loading;
            return;
        }
        self.history.truncate(self.history_position + 1);
        self.history.push(url.clone());
        self.history_position = self.history.len() - 1;
        self.url = Some(url);
        self.status = TabStatus::Loading;
    }

    /// Applies a validated `NavigationState` report coming over IPC. The
    /// committed url is synced into the tab's own history and back/forward
    /// derive from the resulting position. The page never gets to assert
    /// these flags: scripts cannot be trusted to report history state.
    pub fn apply_state(&mut self, url: Option<url::Url>, title: &str, is_loading: bool) {
        if let Some(url) = url {
            self.sync_history(url);
        }
        if !title.is_empty() {
            self.title = title.to_string();
        }
        self.status = if is_loading {
            TabStatus::Loading
        } else if self.url.is_some() {
            TabStatus::Loaded
        } else {
            TabStatus::New
        };
    }

    /// Moves to `url` whether it is an existing entry (reload, back or
    /// forward traversal, repeated load/pageshow reports of one page) or a
    /// new one (link click, pushState). One rule covers all reports, so a
    /// page emitting several events for one commit cannot dupe entries.
    fn sync_history(&mut self, url: url::Url) {
        if let Some(position) = self.history.iter().rposition(|entry| *entry == url) {
            self.history_position = position;
        } else {
            self.history.truncate(self.history_position + 1);
            self.history.push(url.clone());
            self.history_position = self.history.len() - 1;
        }
        self.url = Some(url);
    }

    #[cfg(test)]
    pub(crate) fn history_len(&self) -> usize {
        self.history.len()
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
        assert!(!tab.can_go_back());
        assert!(!tab.can_go_forward());
        assert_eq!(tab.display_title(), "New Tab");
    }

    #[test]
    fn begin_navigation_sets_loading_and_url() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://phantom.app/").unwrap();
        tab.begin_navigation(url.clone());
        assert_eq!(tab.status, TabStatus::Loading);
        assert_eq!(tab.url, Some(url));
        assert!(!tab.can_go_back(), "the first entry has no past");
    }

    #[test]
    fn apply_state_commits_navigation_report() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://example.net/doc").unwrap();
        tab.begin_navigation(url.clone());
        tab.apply_state(Some(url.clone()), "Example", false);
        assert_eq!(tab.status, TabStatus::Loaded);
        assert_eq!(tab.title, "Example");
        assert_eq!(tab.display_title(), "Example");
        assert!(!tab.can_go_back(), "committed the entry it already sat on");
        assert!(!tab.can_go_forward());
        assert_eq!(tab.history_len(), 1);
    }

    #[test]
    fn apply_state_keeps_title_while_loading() {
        let mut tab = fresh_tab();
        let url = url::Url::parse("https://example.net/doc").unwrap();
        tab.begin_navigation(url.clone());
        tab.apply_state(Some(url), "", true);
        assert_eq!(tab.status, TabStatus::Loading);
        assert_eq!(tab.display_title(), "example.net");
    }

    #[test]
    fn back_and_forward_availability_follow_committed_reports() {
        let mut tab = fresh_tab();
        let a = url::Url::parse("https://example.net/a").unwrap();
        let b = url::Url::parse("https://example.net/b").unwrap();
        tab.begin_navigation(a.clone());
        // an in-page link commits a new entry: back exists, forward does not
        tab.apply_state(Some(b.clone()), "", false);
        assert!(tab.can_go_back());
        assert!(!tab.can_go_forward());
        // the user goes back; the traversal report lands on the first entry
        tab.apply_state(Some(a.clone()), "", false);
        assert!(!tab.can_go_back());
        assert!(tab.can_go_forward());
        // forward again
        tab.apply_state(Some(b.clone()), "", false);
        assert!(tab.can_go_back());
        assert!(!tab.can_go_forward());
    }

    #[test]
    fn repeated_reports_of_one_commit_do_not_dupe_history() {
        let mut tab = fresh_tab();
        let a = url::Url::parse("https://example.net/a").unwrap();
        let b = url::Url::parse("https://example.net/b").unwrap();
        tab.begin_navigation(a.clone());
        // one navigation commit fires load, pageshow and readystatechange
        tab.apply_state(Some(b.clone()), "", true);
        tab.apply_state(Some(b.clone()), "B", false);
        tab.apply_state(Some(b.clone()), "B", false);
        assert_eq!(tab.history_len(), 2, "a + b, nothing more");
        assert!(tab.can_go_back());
        assert!(!tab.can_go_forward());
    }

    #[test]
    fn push_state_appends_and_truncates_the_forward_stack() {
        let mut tab = fresh_tab();
        let a = url::Url::parse("https://example.net/a").unwrap();
        let b = url::Url::parse("https://example.net/b").unwrap();
        let c = url::Url::parse("https://example.net/c").unwrap();
        tab.begin_navigation(a.clone());
        tab.apply_state(Some(b.clone()), "", false);
        tab.apply_state(Some(a.clone()), "", false); // back
        tab.apply_state(Some(c.clone()), "", false); // pushState replaces forward
        assert!(
            !tab.can_go_forward(),
            "forward entries are gone after a push"
        );
        assert!(tab.can_go_back());
        assert_eq!(tab.history_len(), 2, "a + c, the b entry was truncated");
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
