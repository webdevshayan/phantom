use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::layout::{PaneId, SplitDirection};
use crate::domain::search::SearchEngine;
use crate::domain::settings::{FeatureFlag, Theme};
use crate::domain::space::SpaceId;
use crate::domain::tab::TabId;

/// Schemes a top-level navigation may use. `phantom` serves the built-in
/// internal pages; everything else — most notably `javascript`, `data`,
/// `blob`, `vbscript` — is refused before it ever reaches a webview.
pub const ALLOWED_SCHEMES: &[&str] = &["http", "https", "file", "phantom"];

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum NavigationError {
    #[error("top-level navigation scheme is not allowed")]
    SchemeNotAllowed,
    #[error("value is not a usable url")]
    InvalidUrl,
}

pub fn validate_navigation(url: &Url) -> Result<(), NavigationError> {
    if ALLOWED_SCHEMES.contains(&url.scheme()) {
        Ok(())
    } else {
        Err(NavigationError::SchemeNotAllowed)
    }
}

/// Where a navigation should land.
#[derive(Clone, Debug)]
pub enum NavigationTarget {
    Url(Url),
    Search { query: String, engine: SearchEngine },
    NewTabPage,
}

impl NavigationTarget {
    pub fn url(&self) -> Option<&Url> {
        match self {
            NavigationTarget::Url(url) => Some(url),
            _ => None,
        }
    }
}

/// Everything the chrome can ask the session to do. The session validates
/// every command — ids, urls, feature flags — before mutating state or
/// emitting effects, so UI bugs cannot bypass the model.
#[derive(Clone, Debug)]
pub enum BrowserCommand {
    Navigate { tab: TabId, target: NavigationTarget },
    GoBack { tab: TabId },
    GoForward { tab: TabId },
    Reload { tab: TabId },
    StopLoading { tab: TabId },
    NewTab { space: SpaceId, target: NavigationTarget },
    CloseTab { tab: TabId },
    ActivateTab { tab: TabId },
    NewWindow { space: SpaceId },
    DragOutTab { tab: TabId },
    SplitPane { pane: PaneId, direction: SplitDirection, target: NavigationTarget },
    ClosePane { pane: PaneId },
    FocusPane { pane: PaneId },
    ToggleDevTools { tab: TabId },
    OpenSettings,
    SetTheme { theme: Theme },
    SetSearchEngine { engine: SearchEngine },
    ToggleFlag { flag: FeatureFlag, enabled: bool },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_allowlist() {
        for good in ["http://a.b", "https://example.com", "file:///tmp/x", "phantom://newtab"] {
            let url = Url::parse(good).unwrap();
            assert_eq!(validate_navigation(&url), Ok(()), "{good}");
        }
        for bad in [
            "javascript:alert(1)",
            "data:text/html,<b>",
            "blob:https://x/y",
            "vbscript:x",
            "ftp://files.example.net",
            "chrome://settings",
        ] {
            let url = Url::parse(bad).unwrap();
            assert_eq!(
                validate_navigation(&url),
                Err(NavigationError::SchemeNotAllowed),
                "{bad}"
            );
        }
    }

    #[test]
    fn invalid_url_is_reported_as_such() {
        assert!(Url::parse("not a url at all").is_err());
    }

    #[test]
    fn target_exposes_url_only_for_direct_navigation() {
        let url = Url::parse("https://example.com/").unwrap();
        assert!(NavigationTarget::Url(url.clone()).url().is_some());
        assert!(NavigationTarget::NewTabPage.url().is_none());
        assert!(NavigationTarget::Search {
            query: "q".into(),
            engine: SearchEngine::Google
        }
        .url()
        .is_none());
    }
}
