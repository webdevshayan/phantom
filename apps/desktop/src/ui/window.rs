//! The browser window entity: owns the webview host for its window, renders
//! the chrome (tab strip, toolbar, menu), reconciles webview surfaces with
//! the session layout, and executes session effects.

use std::path::PathBuf;

use gpui::{
    div, prelude::*, Context, FocusHandle, IntoElement, ParentElement, Pixels, Point, Render,
    Styled, Window,
};

use crate::browser::{Effect, WindowId, WindowState};
use crate::domain::command::{BrowserCommand, NavigationTarget};
use crate::domain::layout::{PaneId, RectPx, SplitDirection};
use crate::domain::omnibox::OmniboxBuffer;
use crate::domain::search::SearchEngine;
use crate::domain::settings::Theme;
use crate::domain::space::SpaceId;
use crate::domain::tab::TabId;
use crate::ui::actions;
use crate::ui::app::{
    drain_host_events, drain_settings_queue, lock, open_settings_window, BrowserShell,
};
use crate::ui::tab_strip;
use crate::ui::theme;
use crate::ui::toolbar;
use crate::webview::host::WebviewHost;

/// Item of the pending drag gesture: the pressed tab and its press point.
type DragPress = (TabId, Point<Pixels>);

pub struct BrowserWindow {
    pub window_id: WindowId,
    pub host: WebviewHost,
    pub omnibox: OmniboxBuffer,
    pub omnibox_focus: FocusHandle,
    root_focus: FocusHandle,
    pub menu_open: bool,
    pub drag_press: Option<DragPress>,
    last_title: Option<String>,
    last_url: Option<String>,
    focused_once: bool,
}

impl BrowserWindow {
    pub fn new(window_id: WindowId, profile: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            window_id,
            host: WebviewHost::new(profile, Theme::Light, SearchEngine::default()),
            omnibox: OmniboxBuffer::new(),
            omnibox_focus: cx.focus_handle(),
            root_focus: cx.focus_handle(),
            menu_open: false,
            drag_press: None,
            last_title: None,
            last_url: None,
            focused_once: false,
        }
    }

    // ---- session helpers ----

    pub(crate) fn active_tab_id(&self, cx: &Context<Self>) -> Option<TabId> {
        let shell = cx.global::<BrowserShell>();
        let session = lock(&shell.session);
        session
            .window(self.window_id)
            .and_then(|window| window.active_tab_id())
    }

    pub(crate) fn focused_pane_id(&self, cx: &Context<Self>) -> Option<PaneId> {
        let shell = cx.global::<BrowserShell>();
        let session = lock(&shell.session);
        session
            .window(self.window_id)
            .map(|window| window.focused_pane)
    }

    /// Runs one session command from this window and executes the effects
    /// that belong to it. A rejected command (unknown id, disabled flag, bad
    /// url) is a no-op on the chrome; the model stays consistent.
    pub(crate) fn run_command(
        &mut self,
        command: BrowserCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let effects = {
            let shell = cx.global::<BrowserShell>();
            let mut session = lock(&shell.session);
            match session.handle(command) {
                Ok(effects) => effects,
                Err(_rejected) => return,
            }
        };
        self.execute_effects(effects, window, cx);
        cx.notify();
    }

    /// Applies an ad-hoc factory to the active tab and runs the command.
    pub(crate) fn command_on_active_tab<F>(
        &mut self,
        make: F,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) where
        F: FnOnce(TabId) -> BrowserCommand,
    {
        let Some(tab) = self.active_tab_id(cx) else {
            return;
        };
        self.run_command(make(tab), window, cx);
    }

    pub(crate) fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.run_command(
            BrowserCommand::NewTab {
                space: SpaceId::default_space(),
                target: NavigationTarget::NewTabPage,
            },
            window,
            cx,
        );
    }

    pub(crate) fn split(
        &mut self,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.focused_pane_id(cx) else {
            return;
        };
        self.run_command(
            BrowserCommand::SplitPane {
                pane,
                direction,
                target: NavigationTarget::NewTabPage,
            },
            window,
            cx,
        );
    }

    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        open_settings_window(cx);
        cx.notify();
    }

    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        {
            let shell = cx.global::<BrowserShell>();
            let mut session = lock(&shell.session);
            let _ = session.quit();
        }
        cx.quit();
    }

    // ---- effects ----

    fn execute_effects(
        &mut self,
        effects: Vec<Effect>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for effect in effects {
            match effect {
                Effect::AttachWebview {
                    window: owner,
                    tab,
                    space,
                    url,
                } if owner == self.window_id => {
                    let sender = self.events_sender(cx);
                    let _ = self.host.attach(window, tab, &space, &url, sender);
                }
                Effect::LoadUrl {
                    window: owner,
                    tab,
                    url,
                } if owner == self.window_id => {
                    let _ = self.host.navigate(tab, &url);
                }
                Effect::DetachWebview { tab } => {
                    self.host.detach(tab);
                }
                Effect::GoBack { tab } => {
                    let _ = self.host.go_back(tab);
                }
                Effect::GoForward { tab } => {
                    let _ = self.host.go_forward(tab);
                }
                Effect::Reload { tab } => {
                    let _ = self.host.reload(tab);
                }
                Effect::StopLoading { tab } => {
                    let _ = self.host.stop_loading(tab);
                }
                Effect::ToggleDevTools { tab } => {
                    self.host.toggle_devtools(tab);
                }
                Effect::OpenWindow {
                    window: new_id,
                    bounds,
                    maximized,
                } => {
                    let profile = cx.global::<BrowserShell>().profile.clone();
                    crate::ui::app::open_browser_window(cx, new_id, bounds, maximized, profile);
                }
                Effect::CloseWindow { window: id } if id == self.window_id => {
                    window.remove_window();
                }
                Effect::SetWindowTitle { .. }
                | Effect::ShowSettingsWindow
                | Effect::CloseWindow { .. }
                | Effect::AttachWebview { .. }
                | Effect::LoadUrl { .. } => {
                    // titles are re-read from the session on every render;
                    // other windows reconcile their own surfaces; their
                    // close flows run through their own effect execution
                }
            }
        }
    }

    fn events_sender(
        &self,
        cx: &Context<Self>,
    ) -> std::sync::mpsc::Sender<crate::webview::host::HostEvent> {
        let shell = cx.global::<BrowserShell>();
        shell.events_tx.clone()
    }

    // ---- webview reconciliation ----

    /// Brings the native webview surfaces in line with the session layout:
    /// attach missing surfaces for visible tabs, detach surfaces of closed
    /// tabs, and position the focused pane's tab over the content area.
    fn sync_surfaces(
        &mut self,
        state: &WindowState,
        rects: &[(PaneId, RectPx)],
        window: &mut Window,
        cx: &Context<Self>,
    ) {
        for (_pane, tab_id) in state.layout.pane_tab_pairs() {
            if !self.host.has(tab_id) {
                if let Some(tab) = state.tab(tab_id) {
                    let url = tab
                        .url
                        .clone()
                        .unwrap_or_else(crate::webview::internal_pages::new_tab_url);
                    let space = tab.space.clone();
                    let sender = self.events_sender(cx);
                    let _ = self.host.attach(window, tab_id, &space, &url, sender);
                }
            }
        }
        for attached in self.host.attached_tabs() {
            if state.tabs.iter().all(|tab| tab.id != attached) {
                self.host.detach(attached);
            }
        }
        let focused = state.layout.tab_for_pane(state.focused_pane);
        let hide: Vec<TabId> = state
            .tabs
            .iter()
            .map(|tab| tab.id)
            .filter(|id| Some(*id) != focused)
            .collect();
        if let Some(tab_id) = focused {
            if let Some((_, rect)) = rects.iter().find(|(pane, _)| *pane == state.focused_pane) {
                // pane rects are content-area relative; surfaces are
                // window-relative, so the chrome height shifts them down
                let shifted = RectPx {
                    y: rect.y + theme::CHROME_HEIGHT,
                    ..*rect
                };
                let _ = self.host.show_tab(tab_id, shifted, &hide);
            }
        }
    }

    // ---- key context ----

    /// Focuses the omnibox with the current url selected for retyping.
    pub(crate) fn focus_omnibox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibox.focus(true);
        window.focus(&self.omnibox_focus);
        cx.notify();
    }
}

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            self.root_focus.focus(window);
        }

        // session upkeep first: webview reports and settings pushed by the
        // settings window are applied before anything is drawn
        drain_host_events(cx);
        drain_settings_queue(cx);

        // snapshot what the chrome needs, then drop the session lock
        let (state, settings_copy) = {
            let shell = cx.global::<BrowserShell>();
            let session = lock(&shell.session);
            (
                session.window(self.window_id).cloned(),
                session.settings().clone(),
            )
        };
        let Some(state) = state else {
            return div().id("browser-empty").size_full().bg(gpui::black());
        };
        let theme = theme::resolve(settings_copy.theme, theme::os_prefers_dark(window));
        self.host
            .set_chrome(settings_copy.theme, settings_copy.search_engine);

        // the window title follows the active tab
        let title = state
            .active_tab()
            .map(|tab| tab.display_title().to_string());
        if title.is_some() && title != self.last_title {
            let title = title.expect("checked above");
            window.set_window_title(&title);
            self.last_title = Some(title);
        }

        // the omnibox mirrors the active url while it is not being edited
        let active_url = state.active_tab().and_then(|tab| tab.url.clone());
        let url_text = active_url.as_ref().map(|url| url.to_string());
        if !self.omnibox.is_focused() && self.last_url != url_text {
            self.omnibox.show_url(active_url.as_ref());
            self.last_url = url_text;
        }

        // content geometry in logical pixels, window-relative
        let scale = window.scale_factor();
        let viewport = window.viewport_size();
        let content_width = f32::from(viewport.width / scale).max(0.0);
        let content_height = (f32::from(viewport.height / scale) - theme::CHROME_HEIGHT).max(0.0);
        let rects = state.layout.compute_rects(content_width, content_height);

        self.sync_surfaces(&state, &rects, window, cx);

        div()
            .id("browser-root")
            .key_context("Browser")
            .track_focus(&self.root_focus)
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(theme.window_background)
            .text_color(theme.text)
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::NewTab, window, cx| {
                    this.new_tab(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::CloseTab, window, cx| {
                    this.command_on_active_tab(|tab| BrowserCommand::CloseTab { tab }, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::NewWindow, window, cx| {
                    this.run_command(
                        BrowserCommand::NewWindow {
                            space: SpaceId::default_space(),
                        },
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::FocusOmnibox, window, cx| {
                    this.focus_omnibox(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::GoBack, window, cx| {
                    this.command_on_active_tab(|tab| BrowserCommand::GoBack { tab }, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::GoForward, window, cx| {
                    this.command_on_active_tab(|tab| BrowserCommand::GoForward { tab }, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::Reload, window, cx| {
                    this.command_on_active_tab(|tab| BrowserCommand::Reload { tab }, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::StopLoading, window, cx| {
                    this.command_on_active_tab(
                        |tab| BrowserCommand::StopLoading { tab },
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::DevTools, window, cx| {
                    this.command_on_active_tab(
                        |tab| BrowserCommand::ToggleDevTools { tab },
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::OpenSettings, _window, cx| {
                    this.open_settings(cx);
                }),
            )
            .on_action(cx.listener(
                |this: &mut Self, _a: &actions::SplitHorizontal, window, cx| {
                    this.split(SplitDirection::Horizontal, window, cx);
                },
            ))
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::SplitVertical, window, cx| {
                    this.split(SplitDirection::Vertical, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::ClosePane, window, cx| {
                    let pane = this.focused_pane_id(cx);
                    if let Some(pane) = pane {
                        this.run_command(BrowserCommand::ClosePane { pane }, window, cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::FocusNextPane, window, cx| {
                    let shell = cx.global::<BrowserShell>();
                    let next = {
                        let session = lock(&shell.session);
                        session
                            .window(this.window_id)
                            .map(|window| window.layout.next_pane(window.focused_pane))
                    };
                    let _ = shell;
                    if let Some(pane) = next {
                        this.run_command(BrowserCommand::FocusPane { pane }, window, cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _a: &actions::Quit, _window, cx| {
                    this.quit(cx);
                }),
            )
            .child(tab_strip::render(self, &theme, window, cx))
            .child(toolbar::render(self, &theme, window, cx))
            .child(
                // spacer under the native webview surfaces
                div().flex_1().bg(theme.window_background),
            )
    }
}
