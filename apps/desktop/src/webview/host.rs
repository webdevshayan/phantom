//! wry-backed webview hosting, Windows-first.
//!
//! Isolation model: every space owns a `WebContext` rooted at
//! `<profile>/spaces/<space>`, so cookies, site storage and caches live in
//! separate directories and never cross spaces — two spaces visiting the
//! same site are different storage origins.
//!
//! Surfaces are keyed by tab: one native webview per tab, shown and
//! positioned over the pane that currently displays it, hidden otherwise.
//! Tab history therefore survives pane swaps and split closes.
//!
//! Linux note: wry's Linux backend rides on webkit2gtk and needs a GTK main
//! loop, which the GPUI shell does not run; Phantom targets Windows
//! (WebView2 + Win32) for phase 1.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use raw_window_handle::HasWindowHandle;
use wry::WebContext;

use crate::domain::layout::RectPx;
use crate::domain::search::SearchEngine;
use crate::domain::settings::Theme;
use crate::domain::space::SpaceId;
use crate::domain::tab::TabId;
use crate::infra::ipc::{self, WebviewToHost};
use crate::webview::bridge::STATE_BRIDGE_JS;
use crate::webview::internal_pages;

/// Pre-validated events flowing from webviews back to the session. Only
/// messages that passed `infra::ipc::decode_webview_message` end up here.
#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    NavigationStateChanged { tab: TabId, state: ipc::NavigationState },
    OpenInNewTab { tab: TabId, url: String, background: bool },
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("webview: {0}")]
    Wry(#[from] wry::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct WebviewHost {
    profile: PathBuf,
    theme: Theme,
    engine: SearchEngine,
    /// one native surface per tab
    webviews: HashMap<TabId, wry::WebView>,
    /// one web context per space — this is the isolation boundary
    web_contexts: HashMap<SpaceId, WebContext>,
}

impl WebviewHost {
    pub fn new(profile: PathBuf, theme: Theme, engine: SearchEngine) -> Self {
        Self {
            profile,
            theme,
            engine,
            webviews: HashMap::new(),
            web_contexts: HashMap::new(),
        }
    }

    /// Applies settings changes to pages served later (internal pages).
    pub fn set_chrome(&mut self, theme: Theme, engine: SearchEngine) {
        self.theme = theme;
        self.engine = engine;
    }

    /// Creates the tab's webview surface parented to `parent`, rooted in the
    /// space's private data directory, loading `initial_url`.
    pub fn attach<W: HasWindowHandle>(
        &mut self,
        parent: &W,
        tab: TabId,
        space: &SpaceId,
        initial_url: &url::Url,
        events: Sender<HostEvent>,
    ) -> Result<(), HostError> {
        if self.webviews.contains_key(&tab) {
            return Ok(());
        }
        let data_dir = crate::infra::paths::space_data_dir(&self.profile, space);
        std::fs::create_dir_all(&data_dir)?;
        // take the context out so the builder can hold it mutably for the
        // duration of the build, then put it back once the borrow ends
        let mut context = self
            .web_contexts
            .remove(space)
            .unwrap_or_else(|| WebContext::new(Some(data_dir)));
        let sender = events.clone();
        let event_tab = tab;
        let theme = self.theme;
        let engine = self.engine;
        let webview = wry::WebViewBuilder::new_with_web_context(&mut context)
            .with_url(initial_url.to_string())
            .with_initialization_script(STATE_BRIDGE_JS)
            .with_transparent(false)
            .with_devtools(true)
            .with_navigation_handler(|url| {
                // defense in depth: never hand a top-level `data:` navigation
                // to the engine; internal pages come from the custom protocol
                !url.starts_with("data:")
            })
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let raw = request.body().as_str();
                if let Ok(message) = ipc::decode_webview_message(raw) {
                    let event = match message {
                        WebviewToHost::NavigationStateChanged { state } => {
                            HostEvent::NavigationStateChanged { tab: event_tab, state }
                        }
                        WebviewToHost::OpenInNewTab { url, background } => {
                            HostEvent::OpenInNewTab { tab: event_tab, url, background }
                        }
                    };
                    // a closed receiver must never crash the webview thread
                    let _ = sender.send(event);
                }
            })
            .with_custom_protocol("phantom".into(), move |_webview_id, request| {
                serve_internal_page(request, theme, engine)
            })
            .build_as_child(parent)?;
        self.web_contexts.insert(space.clone(), context);
        self.webviews.insert(tab, webview);
        Ok(())
    }

    pub fn navigate(&self, tab: TabId, url: &url::Url) -> Result<(), HostError> {
        if let Some(webview) = self.webviews.get(&tab) {
            webview.load_url(url.as_str())?;
        }
        Ok(())
    }

    pub fn go_back(&self, tab: TabId) -> Result<(), HostError> {
        if let Some(webview) = self.webviews.get(&tab) {
            webview.go_back()?;
        }
        Ok(())
    }

    pub fn go_forward(&self, tab: TabId) -> Result<(), HostError> {
        if let Some(webview) = self.webviews.get(&tab) {
            webview.go_forward()?;
        }
        Ok(())
    }

    pub fn reload(&self, tab: TabId) -> Result<(), HostError> {
        if let Some(webview) = self.webviews.get(&tab) {
            webview.reload()?;
        }
        Ok(())
    }

    pub fn stop_loading(&self, tab: TabId) -> Result<(), HostError> {
        if let Some(webview) = self.webviews.get(&tab) {
            webview.evaluate_script("window.stop(); void 0;")?;
        }
        Ok(())
    }

    /// Shows `tab`'s surface inside `rect` (window-relative logical pixels)
    /// and hides every tab in `hide`. Hidden surfaces stay alive so their
    /// history is not lost.
    pub fn show_tab(&self, tab: TabId, rect: RectPx, hide: &[TabId]) -> Result<(), HostError> {
        for other in hide {
            if let Some(webview) = self.webviews.get(other) {
                webview.set_visible(false)?;
            }
        }
        if let Some(webview) = self.webviews.get(&tab) {
            webview.set_bounds(logical_rect(rect))?;
            webview.set_visible(true)?;
        }
        Ok(())
    }

    pub fn focus_tab(&self, tab: TabId) -> Result<(), HostError> {
        if let Some(webview) = self.webviews.get(&tab) {
            webview.focus()?;
        }
        Ok(())
    }

    /// Destroys the surface of a closed tab.
    pub fn detach(&mut self, tab: TabId) -> bool {
        self.webviews.remove(&tab).is_some()
    }

    pub fn attached_tabs(&self) -> Vec<TabId> {
        let mut tabs: Vec<TabId> = self.webviews.keys().copied().collect();
        tabs.sort();
        tabs
    }

    /// F12 / menu entry. On Windows WebView2 manages the devtools window
    /// lifecycle, so closing is left to it.
    pub fn toggle_devtools(&self, tab: TabId) {
        if let Some(webview) = self.webviews.get(&tab) {
            #[cfg(not(target_os = "windows"))]
            {
                if webview.is_devtools_open() {
                    webview.close_devtools();
                } else {
                    webview.open_devtools();
                }
            }
            #[cfg(target_os = "windows")]
            {
                webview.open_devtools();
            }
        }
    }
}

fn logical_rect(rect: RectPx) -> wry::Rect {
    wry::Rect {
        position: wry::Position::Logical(wry::dpi::LogicalPosition::new(
            f64::from(rect.x),
            f64::from(rect.y),
        )),
        size: wry::Size::Logical(wry::dpi::LogicalSize::new(
            f64::from(rect.width),
            f64::from(rect.height),
        )),
    }
}

/// Serves `phantom://` pages from static markup with fixed headers.
fn serve_internal_page(
    request: wry::http::Request<Vec<u8>>,
    theme: Theme,
    engine: SearchEngine,
) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    use std::borrow::Cow;
    let path = request.uri().path().trim_start_matches('/').to_string();
    let (status, html) = match path.as_str() {
        "newtab" => (200, internal_pages::new_tab_page(engine, theme)),
        "blank" => (200, internal_pages::blank_page()),
        _ => (404, internal_pages::error_page("This internal page does not exist")),
    };
    wry::http::Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .body(Cow::Owned(html.into_bytes()))
        .expect("static response builds")
}
