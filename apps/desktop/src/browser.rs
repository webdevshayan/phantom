//! Session controller: the state machine between the UI chrome, the webview
//! host and persisted state.
//!
//! It validates every command (ids, urls, feature flags), owns no toolkit
//! handles, and only emits plain [`Effect`]s for the adapters to run. That
//! keeps the whole flow unit-testable on a bare toolchain.

use url::Url;

use crate::domain::command::{
    validate_navigation, BrowserCommand, NavigationError, NavigationTarget,
};
use crate::domain::layout::{LayoutTree, PaneId, SplitDirection};
use crate::domain::search::SearchEngine;
use crate::domain::settings::{FeatureDisabledError, FeatureFlag, Settings, Theme};
use crate::domain::space::SpaceId;
use crate::domain::tab::{Tab, TabId};
use crate::infra::db::{
    DbError, SessionSnapshot, Store, TabSnapshot, WindowBounds, WindowSnapshot,
};
use crate::infra::ipc::NavigationState;
use crate::infra::settings_store;
use crate::webview::internal_pages;

pub type WindowId = u64;

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("tab {0:?} is not part of this session")]
    UnknownTab(TabId),
    #[error("pane {0:?} is not part of this session")]
    UnknownPane(PaneId),
    #[error("window {0} is not part of this session")]
    UnknownWindow(WindowId),
    #[error("no window is open")]
    NoWindow,
    #[error("feature '{0}' is disabled in settings")]
    FeatureDisabled(&'static str),
    #[error(transparent)]
    Navigation(#[from] NavigationError),
    #[error(transparent)]
    Layout(#[from] crate::domain::layout::LayoutError),
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<FeatureDisabledError> for ModelError {
    fn from(error: FeatureDisabledError) -> Self {
        ModelError::FeatureDisabled(error.0)
    }
}

/// One browser window: its tab strip, split layout and geometry.
#[derive(Clone, Debug)]
pub struct WindowState {
    pub id: WindowId,
    pub bounds: WindowBounds,
    pub maximized: bool,
    pub tabs: Vec<Tab>,
    pub layout: LayoutTree,
    pub focused_pane: PaneId,
}

impl WindowState {
    pub fn pane_count(&self) -> usize {
        self.layout.pane_count()
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// The tab shown in the focused pane — what the omnibox edits.
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.layout.tab_for_pane(self.focused_pane)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab_id().and_then(|id| self.tab(id))
    }

    pub fn pane_of_tab(&self, id: TabId) -> Option<PaneId> {
        self.layout
            .pane_tab_pairs()
            .into_iter()
            .find(|(_, tab)| *tab == id)
            .map(|(pane, _)| pane)
    }
}

/// Side effects the adapters (GPUI app, wry host) must execute after a
/// command mutates session state. The session never touches toolkits.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Open a native window; `bounds == None` means platform default.
    OpenWindow { window: WindowId, bounds: Option<WindowBounds>, maximized: bool },
    CloseWindow { window: WindowId },
    /// Create the webview surface for `tab`, isolated to `space`, loading `url`.
    AttachWebview { window: WindowId, tab: TabId, space: SpaceId, url: Url },
    /// Destroy the webview surface of `tab`.
    DetachWebview { tab: TabId },
    LoadUrl { window: WindowId, tab: TabId, url: Url },
    GoBack { tab: TabId },
    GoForward { tab: TabId },
    Reload { tab: TabId },
    StopLoading { tab: TabId },
    ToggleDevTools { tab: TabId },
    SetWindowTitle { window: WindowId, title: String },
    ShowSettingsWindow,
}

pub struct PhantomSession {
    store: Store,
    settings: Settings,
    windows: Vec<WindowState>,
    focused_window: Option<WindowId>,
    next_window_id: u64,
    next_tab_id: u64,
    next_pane_id: u64,
}

impl PhantomSession {
    pub fn new(store: Store, settings: Settings) -> Self {
        Self {
            store,
            settings,
            windows: Vec::new(),
            focused_window: None,
            next_window_id: 1,
            next_tab_id: 1,
            next_pane_id: 1,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn windows(&self) -> &[WindowState] {
        &self.windows
    }

    pub fn window(&self, id: WindowId) -> Option<&WindowState> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn focused_window_id(&self) -> Option<WindowId> {
        self.focused_window
    }

    pub fn focus_window(&mut self, id: WindowId) -> Result<(), ModelError> {
        if self.windows.iter().any(|window| window.id == id) {
            self.focused_window = Some(id);
            Ok(())
        } else {
            Err(ModelError::UnknownWindow(id))
        }
    }

    fn window_mut(&mut self, id: WindowId) -> Option<&mut WindowState> {
        self.windows.iter_mut().find(|window| window.id == id)
    }

    fn window_of_tab(&self, tab: TabId) -> Option<&WindowState> {
        self.windows.iter().find(|window| window.tab(tab).is_some())
    }

    fn window_of_pane(&self, pane: PaneId) -> Option<&WindowState> {
        self.windows
            .iter()
            .find(|window| window.layout.contains_pane(pane))
    }

    fn alloc_window_id(&mut self) -> u64 {
        let id = self.next_window_id;
        self.next_window_id += 1;
        id
    }

    fn alloc_tab_id(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        id
    }

    fn alloc_pane_id(&mut self) -> u64 {
        let id = self.next_pane_id;
        self.next_pane_id += 1;
        id
    }

    // ---- lifecycle ----

    /// Restores persisted windows and returns the effects to realize them.
    /// A window whose snapshot fails validation is skipped; a browser that
    /// refuses to launch over one stale record is worse than losing it.
    pub fn restore(&mut self) -> Result<Vec<Effect>, ModelError> {
        let snapshot = self.store.load_session()?;
        let mut effects = Vec::new();
        for window in snapshot.windows {
            if window.validate().is_err() {
                continue;
            }
            let mut tabs = Vec::new();
            for record in window.tabs {
                let mut tab = Tab::new(record.id, record.space);
                tab.title = record.title;
                if let Some(text) = record.url.filter(|url| !url.is_empty()) {
                    if let Ok(url) = Url::parse(&text) {
                        if validate_navigation(&url).is_ok() {
                            tab.url = Some(url);
                        }
                    }
                }
                self.next_tab_id = self.next_tab_id.max(tab.id.0 + 1);
                tabs.push(tab);
            }
            for pane in window.layout.panes() {
                self.next_pane_id = self.next_pane_id.max(pane.0 + 1);
            }
            self.next_window_id = self.next_window_id.max(window.id + 1);
            let focused_pane = window
                .active_tab
                .and_then(|tab| {
                    window
                        .layout
                        .pane_tab_pairs()
                        .into_iter()
                        .find(|(_, shown)| *shown == tab)
                })
                .map(|(pane, _)| pane)
                .unwrap_or_else(|| window.layout.first_pane());
            let mut state = WindowState {
                id: window.id,
                bounds: window.bounds,
                maximized: window.maximized,
                tabs,
                layout: window.layout,
                focused_pane,
            };
            effects.push(Effect::OpenWindow {
                window: state.id,
                bounds: Some(state.bounds),
                maximized: state.maximized,
            });
            let visible: std::collections::HashSet<u64> =
                state.layout.tab_ids().iter().map(|tab| tab.0).collect();
            for (_, tab_id) in state.layout.pane_tab_pairs() {
                let Some(tab) = state.tab(tab_id) else {
                    continue;
                };
                let url = tab.url.clone().unwrap_or_else(internal_pages::new_tab_url);
                let space = tab.space.clone();
                effects.push(Effect::AttachWebview {
                    window: state.id,
                    tab: tab_id,
                    space,
                    url,
                });
            }
            for tab in state.tabs.iter_mut() {
                if visible.contains(&tab.id.0) {
                    tab.surfaced = true;
                }
            }
            self.windows.push(state);
        }
        self.focused_window = self.windows.first().map(|window| window.id);
        Ok(effects)
    }

    /// Guarantees at least one window after launch when nothing was restored.
    pub fn ensure_startup_window(&mut self) -> Result<Vec<Effect>, ModelError> {
        if !self.windows.is_empty() {
            return Ok(Vec::new());
        }
        self.new_window(SpaceId::default_space())
    }

    /// Persists the session snapshot and settings.
    pub fn flush(&mut self) -> Result<(), ModelError> {
        let snapshot = self.snapshot();
        self.store.save_session(&snapshot)?;
        settings_store::save(&mut self.store, &self.settings)?;
        Ok(())
    }

    /// Tears everything down in a clean, persisted order.
    pub fn quit(&mut self) -> Result<Vec<Effect>, ModelError> {
        let mut effects = Vec::new();
        for window in &self.windows {
            for tab in &window.tabs {
                effects.push(Effect::DetachWebview { tab: tab.id });
            }
            effects.push(Effect::CloseWindow { window: window.id });
        }
        self.windows.clear();
        self.focused_window = None;
        self.flush()?;
        Ok(effects)
    }

    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            windows: self
                .windows
                .iter()
                .map(|window| WindowSnapshot {
                    id: window.id,
                    bounds: window.bounds,
                    maximized: window.maximized,
                    active_tab: window.active_tab_id(),
                    layout: window.layout.clone(),
                    tabs: window
                        .tabs
                        .iter()
                        .map(|tab| TabSnapshot {
                            id: tab.id,
                            space: tab.space.clone(),
                            title: tab.title.clone(),
                            url: tab.url.as_ref().map(|url| url.to_string()),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    // ---- command intake ----

    /// Handles one chrome command: validate, mutate, persist, return effects.
    pub fn handle(&mut self, command: BrowserCommand) -> Result<Vec<Effect>, ModelError> {
        let effects = self.apply(command)?;
        self.flush()?;
        Ok(effects)
    }

    fn apply(&mut self, command: BrowserCommand) -> Result<Vec<Effect>, ModelError> {
        match command {
            BrowserCommand::Navigate { tab, target } => self.navigate(tab, target),
            BrowserCommand::GoBack { tab } => self.require_tab(tab, Effect::GoBack { tab }),
            BrowserCommand::GoForward { tab } => self.require_tab(tab, Effect::GoForward { tab }),
            BrowserCommand::Reload { tab } => self.require_tab(tab, Effect::Reload { tab }),
            BrowserCommand::StopLoading { tab } => {
                self.require_tab(tab, Effect::StopLoading { tab })
            }
            BrowserCommand::NewTab { space, target } => self.new_tab(space, target),
            BrowserCommand::CloseTab { tab } => self.close_tab(tab),
            BrowserCommand::ActivateTab { tab } => self.activate_tab(tab),
            BrowserCommand::NewWindow { space } => self.new_window(space),
            BrowserCommand::DragOutTab { tab } => self.drag_out_tab(tab),
            BrowserCommand::SplitPane { pane, direction, target } => {
                self.split_pane(pane, direction, target)
            }
            BrowserCommand::ClosePane { pane } => self.close_pane(pane),
            BrowserCommand::FocusPane { pane } => self.focus_pane(pane),
            BrowserCommand::ToggleDevTools { tab } => {
                self.require_tab(tab, Effect::ToggleDevTools { tab })
            }
            BrowserCommand::OpenSettings => Ok(vec![Effect::ShowSettingsWindow]),
            BrowserCommand::SetTheme { theme } => {
                self.settings.theme = theme;
                Ok(Vec::new())
            }
            BrowserCommand::SetSearchEngine { engine } => {
                self.settings.search_engine = engine;
                Ok(Vec::new())
            }
            BrowserCommand::ToggleFlag { flag, enabled } => {
                self.settings.flags.set(flag, enabled);
                Ok(Vec::new())
            }
        }
    }

    fn require_tab(&self, tab: TabId, effect: Effect) -> Result<Vec<Effect>, ModelError> {
        if self.window_of_tab(tab).is_some() {
            Ok(vec![effect])
        } else {
            Err(ModelError::UnknownTab(tab))
        }
    }

    fn resolve_target(&self, target: NavigationTarget) -> Result<Url, ModelError> {
        match target {
            NavigationTarget::Url(url) => {
                validate_navigation(&url)?;
                Ok(url)
            }
            NavigationTarget::Search { query, engine } => Ok(engine.search_url(&query)),
            NavigationTarget::NewTabPage => Ok(internal_pages::new_tab_url()),
        }
    }

    fn navigate(&mut self, tab_id: TabId, target: NavigationTarget) -> Result<Vec<Effect>, ModelError> {
        let url = self.resolve_target(target)?;
        let window_id = self
            .window_of_tab(tab_id)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        if let Some(tab) = window.tab_mut(tab_id) {
            tab.begin_navigation(url.clone());
        }
        let mut effects = Vec::new();
        if !ensure_surface(window, window_id, tab_id, &mut effects) {
            effects.push(Effect::LoadUrl {
                window: window_id,
                tab: tab_id,
                url,
            });
        }
        Ok(effects)
    }

    fn new_tab(&mut self, space: SpaceId, target: NavigationTarget) -> Result<Vec<Effect>, ModelError> {
        let window_id = self.focused_window_id().ok_or(ModelError::NoWindow)?;
        let url = self.resolve_target(target)?;
        self.add_tab(window_id, space, url, false)
    }

    fn add_tab(
        &mut self,
        window_id: WindowId,
        space: SpaceId,
        url: Url,
        background: bool,
    ) -> Result<Vec<Effect>, ModelError> {
        let tab_id = TabId(self.alloc_tab_id());
        let mut tab = Tab::new(tab_id, space);
        tab.begin_navigation(url);
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        window.tabs.push(tab);
        let mut effects = Vec::new();
        if !background {
            window.layout.set_tab(window.focused_pane, tab_id)?;
            ensure_surface(window, window_id, tab_id, &mut effects);
        }
        Ok(effects)
    }

    fn close_tab(&mut self, tab_id: TabId) -> Result<Vec<Effect>, ModelError> {
        let window_id = self
            .window_of_tab(tab_id)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        let removed_index = window
            .tabs
            .iter()
            .position(|tab| tab.id == tab_id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let after = window.tabs.get(removed_index + 1).map(|tab| tab.id);
        let before = if removed_index > 0 {
            window.tabs.get(removed_index - 1).map(|tab| tab.id)
        } else {
            None
        };
        let neighbor = after.or(before);
        window.tabs.retain(|tab| tab.id != tab_id);
        let mut effects = vec![Effect::DetachWebview { tab: tab_id }];
        if window.tabs.is_empty() {
            // last tab closed: the window goes with it
            effects.push(Effect::CloseWindow { window: window_id });
            self.windows.retain(|window| window.id != window_id);
            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.first().map(|window| window.id);
            }
            return Ok(effects);
        }
        let shown = neighbor.expect("strip is non-empty here");
        for pane in window.layout.panes() {
            if window.layout.tab_for_pane(pane) == Some(tab_id) {
                window.layout.set_tab(pane, shown)?;
                ensure_surface(window, window_id, shown, &mut effects);
            }
        }
        Ok(effects)
    }

    fn activate_tab(&mut self, tab_id: TabId) -> Result<Vec<Effect>, ModelError> {
        let window_id = self
            .window_of_tab(tab_id)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        let focused = window.focused_pane;
        let current = window.layout.tab_for_pane(focused);
        if current == Some(tab_id) {
            return Ok(Vec::new());
        }
        let mut effects = Vec::new();
        let holder = window
            .layout
            .pane_tab_pairs()
            .into_iter()
            .find(|(_, shown)| *shown == tab_id)
            .map(|(pane, _)| pane);
        match holder {
            Some(other) if other != focused => {
                // swap panes so each visible tab stays visible exactly once
                let previous = current.expect("focused pane always shows a tab");
                window.layout.set_tab(other, previous)?;
                window.layout.set_tab(focused, tab_id)?;
            }
            _ => {
                window.layout.set_tab(focused, tab_id)?;
            }
        }
        ensure_surface(window, window_id, tab_id, &mut effects);
        Ok(effects)
    }

    fn new_window(&mut self, space: SpaceId) -> Result<Vec<Effect>, ModelError> {
        let window_id = self.alloc_window_id();
        let pane = PaneId(self.alloc_pane_id());
        let tab_id = TabId(self.alloc_tab_id());
        let mut tab = Tab::new(tab_id, space);
        tab.begin_navigation(internal_pages::new_tab_url());
        let mut state = WindowState {
            id: window_id,
            bounds: WindowBounds::default(),
            maximized: false,
            tabs: vec![tab],
            layout: LayoutTree::pane(pane, tab_id),
            focused_pane: pane,
        };
        let mut effects = vec![Effect::OpenWindow {
            window: window_id,
            bounds: None,
            maximized: false,
        }];
        ensure_surface(&mut state, window_id, tab_id, &mut effects);
        self.windows.push(state);
        self.focused_window = Some(window_id);
        Ok(effects)
    }

    fn drag_out_tab(&mut self, tab_id: TabId) -> Result<Vec<Effect>, ModelError> {
        self.settings.flags.require(FeatureFlag::EnableDragTabs)?;
        let source_id = self
            .window_of_tab(tab_id)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let window = self
            .window_mut(source_id)
            .ok_or(ModelError::UnknownWindow(source_id))?;
        let removed_index = window
            .tabs
            .iter()
            .position(|tab| tab.id == tab_id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let after = window.tabs.get(removed_index + 1).map(|tab| tab.id);
        let before = if removed_index > 0 {
            window.tabs.get(removed_index - 1).map(|tab| tab.id)
        } else {
            None
        };
        let neighbor = after.or(before);
        let mut dragged = window.tab(tab_id).cloned().expect("located tab");
        dragged.surfaced = false; // a native surface cannot reparent across windows
        window.tabs.retain(|tab| tab.id != tab_id);
        let mut effects = vec![Effect::DetachWebview { tab: tab_id }];
        let will_close = window.tabs.is_empty();
        let shown = neighbor;
        if !will_close {
            for pane in window.layout.panes() {
                if window.layout.tab_for_pane(pane) == Some(tab_id) {
                    let replacement = shown.expect("strip is non-empty here");
                    window.layout.set_tab(pane, replacement)?;
                    ensure_surface(window, source_id, replacement, &mut effects);
                }
            }
        }
        if will_close {
            effects.push(Effect::CloseWindow { window: source_id });
            self.windows.retain(|window| window.id != source_id);
            if self.focused_window == Some(source_id) {
                self.focused_window = self.windows.first().map(|window| window.id);
            }
        }
        let new_window_id = self.alloc_window_id();
        let new_pane = PaneId(self.alloc_pane_id());
        let mut state = WindowState {
            id: new_window_id,
            bounds: WindowBounds::default(),
            maximized: false,
            tabs: vec![dragged],
            layout: LayoutTree::pane(new_pane, tab_id),
            focused_pane: new_pane,
        };
        effects.push(Effect::OpenWindow {
            window: new_window_id,
            bounds: None,
            maximized: false,
        });
        ensure_surface(&mut state, new_window_id, tab_id, &mut effects);
        self.windows.push(state);
        self.focused_window = Some(new_window_id);
        Ok(effects)
    }

    fn split_pane(
        &mut self,
        pane: PaneId,
        direction: SplitDirection,
        target: NavigationTarget,
    ) -> Result<Vec<Effect>, ModelError> {
        self.settings.flags.require(FeatureFlag::EnableTiling)?;
        let window_id = self
            .window_of_pane(pane)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownPane(pane))?;
        let url = self.resolve_target(target)?;
        // allocate before taking the window borrow
        let tab_id = TabId(self.alloc_tab_id());
        let new_pane = PaneId(self.alloc_pane_id());
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        let space = window
            .layout
            .tab_for_pane(pane)
            .and_then(|tab| window.tab(tab))
            .map(|tab| tab.space.clone())
            .unwrap_or_else(SpaceId::default_space);
        let mut tab = Tab::new(tab_id, space.clone());
        tab.begin_navigation(url.clone());
        window.tabs.push(tab);
        window.layout = window
            .layout
            .clone()
            .split(pane, direction, new_pane, tab_id)?;
        let mut effects = Vec::new();
        ensure_surface(window, window_id, tab_id, &mut effects);
        Ok(effects)
    }

    fn close_pane(&mut self, pane: PaneId) -> Result<Vec<Effect>, ModelError> {
        let window_id = self
            .window_of_pane(pane)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownPane(pane))?;
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        if window.pane_count() == 1 {
            // the last pane out: window and strip go with it
            let tab_ids: Vec<TabId> = window.tabs.iter().map(|tab| tab.id).collect();
            let mut effects: Vec<Effect> = tab_ids
                .into_iter()
                .map(|tab| Effect::DetachWebview { tab })
                .collect();
            effects.push(Effect::CloseWindow { window: window_id });
            self.windows.retain(|window| window.id != window_id);
            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.first().map(|window| window.id);
            }
            return Ok(effects);
        }
        let was_focused = window.focused_pane == pane;
        window.layout = window
            .layout
            .clone()
            .close(pane)?
            .expect("guarded by the pane_count check");
        if was_focused {
            window.focused_pane = window.layout.first_pane();
        }
        Ok(Vec::new())
    }

    fn focus_pane(&mut self, pane: PaneId) -> Result<Vec<Effect>, ModelError> {
        let window_id = self
            .window_of_pane(pane)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownPane(pane))?;
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        window.focused_pane = pane;
        Ok(Vec::new())
    }

    // ---- webview report intake (pre-validated by the IPC layer) ----

    pub fn report_navigation(
        &mut self,
        tab_id: TabId,
        state: NavigationState,
    ) -> Result<Vec<Effect>, ModelError> {
        let window_id = self
            .window_of_tab(tab_id)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        let url = if state.url.is_empty() {
            None
        } else {
            let url = Url::parse(&state.url)
                .map_err(|_| ModelError::Navigation(NavigationError::InvalidUrl))?;
            validate_navigation(&url)?;
            Some(url)
        };
        let is_active = self
            .window(window_id)
            .and_then(|window| window.active_tab_id())
            .map(|active| active == tab_id)
            .unwrap_or(false);
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        if let Some(tab) = window.tab_mut(tab_id) {
            tab.apply_state(
                url,
                &state.title,
                state.can_go_back,
                state.can_go_forward,
                state.is_loading,
            );
        }
        let mut effects = Vec::new();
        if is_active {
            if let Some(tab) = window.tab(tab_id) {
                effects.push(Effect::SetWindowTitle {
                    window: window_id,
                    title: tab.display_title().to_string(),
                });
            }
        }
        self.flush()?;
        Ok(effects)
    }

    /// Handles a validated `OpenInNewTab` request from a webview.
    pub fn report_open_request(
        &mut self,
        origin: TabId,
        url_text: String,
        background: bool,
    ) -> Result<Vec<Effect>, ModelError> {
        let url = Url::parse(&url_text)
            .map_err(|_| ModelError::Navigation(NavigationError::InvalidUrl))?;
        validate_navigation(&url)?;
        let window_id = self
            .window_of_tab(origin)
            .map(|window| window.id)
            .ok_or(ModelError::UnknownTab(origin))?;
        let space = self
            .window(window_id)
            .and_then(|window| window.tab(origin))
            .map(|tab| tab.space.clone())
            .unwrap_or_else(SpaceId::default_space);
        let effects = self.add_tab(window_id, space, url, background)?;
        self.flush()?;
        Ok(effects)
    }

    /// Geometry reports are hot-path; they update state only and get
    /// persisted at the next flush point (close, quit, structural command).
    pub fn report_window_bounds(
        &mut self,
        window_id: WindowId,
        bounds: WindowBounds,
        maximized: bool,
    ) -> Result<(), ModelError> {
        let window = self
            .window_mut(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        window.bounds = bounds;
        window.maximized = maximized;
        Ok(())
    }

    /// The user closed a native window through its own controls. Returns
    /// detach effects so the adapter can clean up host state; the window
    /// itself is already gone.
    pub fn dismiss_window(&mut self, window_id: WindowId) -> Result<Vec<Effect>, ModelError> {
        let window = self
            .window(window_id)
            .ok_or(ModelError::UnknownWindow(window_id))?;
        let effects: Vec<Effect> = window
            .tabs
            .iter()
            .map(|tab| Effect::DetachWebview { tab: tab.id })
            .collect();
        self.windows.retain(|window| window.id != window_id);
        if self.focused_window == Some(window_id) {
            self.focused_window = self.windows.first().map(|window| window.id);
        }
        self.flush()?;
        Ok(effects)
    }

    /// Applies an update pushed from the settings window and persists it.
    /// `None` fields are left untouched.
    pub fn apply_settings_update(
        &mut self,
        theme: Option<Theme>,
        engine: Option<SearchEngine>,
        flag: Option<(FeatureFlag, bool)>,
    ) -> Result<(), ModelError> {
        if let Some(theme) = theme {
            self.settings.theme = theme;
        }
        if let Some(engine) = engine {
            self.settings.search_engine = engine;
        }
        if let Some((flag, enabled)) = flag {
            self.settings.flags.set(flag, enabled);
        }
        self.flush()?;
        Ok(())
    }
}

/// Emits the effect that guarantees a webview surface for `tab`, marking it
/// surfaced. Returns true when an attach effect was emitted.
fn ensure_surface(
    window: &mut WindowState,
    window_id: WindowId,
    tab_id: TabId,
    effects: &mut Vec<Effect>,
) -> bool {
    let Some(tab) = window.tab_mut(tab_id) else {
        return false;
    };
    if tab.surfaced {
        return false;
    }
    let url = tab.url.clone().unwrap_or_else(internal_pages::new_tab_url);
    let space = tab.space.clone();
    tab.surfaced = true;
    effects.push(Effect::AttachWebview {
        window: window_id,
        tab: tab_id,
        space,
        url,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::layout::SplitDirection;
    use crate::domain::search::SearchEngine;

    fn session() -> PhantomSession {
        let store = Store::open_in_memory().unwrap();
        PhantomSession::new(store, Settings::default())
    }

    fn started() -> PhantomSession {
        let mut session = session();
        session.ensure_startup_window().unwrap();
        session
    }

    fn active_tab_id(session: &PhantomSession) -> Option<TabId> {
        session
            .focused_window_id()
            .and_then(|id| session.window(id))
            .and_then(|window| window.active_tab_id())
    }

    #[test]
    fn startup_opens_one_window_with_a_new_tab_surface() {
        let mut session = session();
        let effects = session.ensure_startup_window().unwrap();
        assert_eq!(effects.len(), 2, "OpenWindow + AttachWebview expected");
        assert_eq!(session.windows().len(), 1);
        assert!(matches!(effects[0], Effect::OpenWindow { .. }));
        match &effects[1] {
            Effect::AttachWebview { space, url, .. } => {
                assert_eq!(space.as_str(), "default");
                assert_eq!(url.as_str(), "phantom://newtab");
            }
            other => panic!("unexpected effect {other:?}"),
        }
    }

    #[test]
    fn navigate_validates_scheme_and_updates_tab() {
        let mut session = started();
        let tab = active_tab_id(&session).unwrap();
        let bad = BrowserCommand::Navigate {
            tab,
            target: NavigationTarget::Url(Url::parse("javascript:alert(1)").unwrap()),
        };
        assert!(matches!(
            session.handle(bad),
            Err(ModelError::Navigation(NavigationError::SchemeNotAllowed))
        ));
        let good = BrowserCommand::Navigate {
            tab,
            target: NavigationTarget::Url(Url::parse("https://example.org/").unwrap()),
        };
        let effects = session.handle(good).unwrap();
        assert_eq!(effects.len(), 1);
        let window = session.window(session.focused_window_id().unwrap()).unwrap();
        let tab_state = window.tab(tab).unwrap();
        assert_eq!(tab_state.url.as_ref().unwrap().as_str(), "https://example.org/");
        assert_eq!(tab_state.status, crate::domain::tab::TabStatus::Loading);
    }

    #[test]
    fn omnibox_search_resolves_through_the_engine() {
        let mut session = started();
        let tab = active_tab_id(&session).unwrap();
        let command = BrowserCommand::Navigate {
            tab,
            target: NavigationTarget::Search {
                query: "rust book".into(),
                engine: SearchEngine::DuckDuckGo,
            },
        };
        session.handle(command).unwrap();
        let window = session.window(session.focused_window_id().unwrap()).unwrap();
        let url = window.tab(tab).unwrap().url.clone().unwrap();
        assert_eq!(url.host_str(), Some("duckduckgo.com"));
    }

    #[test]
    fn new_tab_rebinds_focused_pane_and_close_tab_moves_to_neighbor() {
        let mut session = started();
        let first = active_tab_id(&session).unwrap();
        let effects = session
            .handle(BrowserCommand::NewTab {
                space: SpaceId::default_space(),
                target: NavigationTarget::NewTabPage,
            })
            .unwrap();
        assert_eq!(effects.len(), 1, "attach effect for the new tab");
        let second = active_tab_id(&session).unwrap();
        assert_ne!(first, second);

        let effects = session.handle(BrowserCommand::CloseTab { tab: second }).unwrap();
        assert!(effects.contains(&Effect::DetachWebview { tab: second }));
        assert_eq!(active_tab_id(&session), Some(first));

        // closing the last tab closes the window
        let window_id = session.focused_window_id().unwrap();
        let effects = session.handle(BrowserCommand::CloseTab { tab: first }).unwrap();
        assert!(effects.contains(&Effect::CloseWindow { window: window_id }));
        assert!(session.windows().is_empty());
    }

    #[test]
    fn activate_tab_swaps_panes_instead_of_duplicating() {
        let mut session = started();
        session
            .handle(BrowserCommand::ToggleFlag {
                flag: FeatureFlag::EnableTiling,
                enabled: true,
            })
            .unwrap();
        let window_id = session.focused_window_id().unwrap();
        let pane = session.window(window_id).unwrap().focused_pane;
        session
            .handle(BrowserCommand::SplitPane {
                pane,
                direction: SplitDirection::Horizontal,
                target: NavigationTarget::NewTabPage,
            })
            .unwrap();
        let first = session.window(window_id).unwrap().tabs[0].id;
        let second = session.window(window_id).unwrap().tabs[1].id;
        assert_ne!(first, second);

        // activate the first tab while the focused pane shows the second:
        // the panes swap, both tabs stay visible exactly once
        session.handle(BrowserCommand::ActivateTab { tab: first }).unwrap();
        let pairs = session.window(window_id).unwrap().layout.pane_tab_pairs();
        assert_eq!(
            pairs.iter().filter(|(_, tab)| *tab == first).count(),
            1,
            "first visible exactly once: {pairs:?}"
        );
        assert_eq!(
            pairs.iter().filter(|(_, tab)| *tab == second).count(),
            1,
            "second visible exactly once: {pairs:?}"
        );
    }

    #[test]
    fn tiling_requires_the_flag() {
        let mut session = started();
        let window_id = session.focused_window_id().unwrap();
        let pane = session.window(window_id).unwrap().focused_pane;
        let command = BrowserCommand::SplitPane {
            pane,
            direction: SplitDirection::Vertical,
            target: NavigationTarget::NewTabPage,
        };
        assert!(matches!(
            session.handle(command.clone()),
            Err(ModelError::FeatureDisabled("enable_tiling"))
        ));
        session
            .handle(BrowserCommand::ToggleFlag {
                flag: FeatureFlag::EnableTiling,
                enabled: true,
            })
            .unwrap();
        let effects = session.handle(command).unwrap();
        assert_eq!(effects.len(), 1, "attach for the new pane tab");
        assert_eq!(
            session.window(window_id).unwrap().pane_count(),
            2,
            "tiling enabled → 2 panes"
        );
    }

    #[test]
    fn drag_out_requires_the_flag_and_spawns_a_window() {
        let mut session = started();
        let tab = active_tab_id(&session).unwrap();
        assert!(matches!(
            session.handle(BrowserCommand::DragOutTab { tab }),
            Err(ModelError::FeatureDisabled("enable_drag_tabs"))
        ));
        session
            .handle(BrowserCommand::ToggleFlag {
                flag: FeatureFlag::EnableDragTabs,
                enabled: true,
            })
            .unwrap();
        // a second tab so the source window survives the drag
        session
            .handle(BrowserCommand::NewTab {
                space: SpaceId::default_space(),
                target: NavigationTarget::NewTabPage,
            })
            .unwrap();
        let effects = session.handle(BrowserCommand::DragOutTab { tab }).unwrap();
        assert!(effects.contains(&Effect::DetachWebview { tab }));
        assert!(effects.contains(&Effect::OpenWindow { window: 2, bounds: None, maximized: false }));
        assert_eq!(session.windows().len(), 2);
    }

    #[test]
    fn closing_the_only_pane_closes_window_and_strip() {
        let mut session = started();
        let window_id = session.focused_window_id().unwrap();
        let pane = session.window(window_id).unwrap().focused_pane;
        let tab_count = session.window(window_id).unwrap().tabs.len();
        let effects = session.handle(BrowserCommand::ClosePane { pane }).unwrap();
        assert_eq!(effects.len(), tab_count + 1, "detach per tab + close window");
        assert!(session.windows().is_empty());
    }

    #[test]
    fn background_open_requests_stay_invisible_until_activated() {
        let mut session = started();
        let origin = active_tab_id(&session).unwrap();
        let effects = session
            .report_open_request(
                origin,
                "https://example.net/".to_string(),
                true,
            )
            .unwrap();
        assert!(effects.is_empty());
        let window = session.window(session.focused_window_id().unwrap()).unwrap();
        assert_eq!(window.tabs.len(), 2);
        let background_tab = window.tabs.last().unwrap();
        assert!(!background_tab.surfaced);
        // foreground open surfaces immediately
        let effects = session
            .report_open_request(origin, "https://example.org/".to_string(), false)
            .unwrap();
        assert_eq!(effects.len(), 1);
    }

    #[test]
    fn navigation_reports_update_title_and_back_forward() {
        let mut session = started();
        let tab = active_tab_id(&session).unwrap();
        let effects = session
            .report_navigation(
                tab,
                NavigationState {
                    url: "https://example.com/page".into(),
                    title: "Example Page".into(),
                    can_go_back: true,
                    can_go_forward: false,
                    is_loading: false,
                },
            )
            .unwrap();
        assert_eq!(
            effects,
            vec![Effect::SetWindowTitle {
                window: session.focused_window_id().unwrap(),
                title: "Example Page".into()
            }]
        );
        let window = session.window(session.focused_window_id().unwrap()).unwrap();
        let tab_state = window.tab(tab).unwrap();
        assert!(tab_state.can_go_back);
        assert_eq!(tab_state.status, crate::domain::tab::TabStatus::Loaded);
    }

    #[test]
    fn quit_detaches_and_clears() {
        let mut session = started();
        let effects = session.quit().unwrap();
        assert_eq!(effects.len(), 2, "detach + close window");
        assert!(session.windows().is_empty());
    }
}
