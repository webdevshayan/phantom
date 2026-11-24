//! Top tab strip: one strip per window, tear-off drag gesture gated by the
//! `enable_drag_tabs` feature flag.

use gpui::{
    div, prelude::*, px, Context, Div, MouseDownEvent, MouseMoveEvent, MouseButton, SharedString,
    Styled, Window,
};

use crate::browser::BrowserWindow;
use crate::domain::command::BrowserCommand;
use crate::domain::settings::FeatureFlag;
use crate::domain::tab::{Tab, TabId};
use crate::ui::app::{lock, BrowserShell};
use crate::ui::theme::{ChromeTheme, TAB_STRIP_HEIGHT};

/// Pixels of movement past which a held tab tears off into a new window.
const DRAG_THRESHOLD: f32 = 24.0;

pub fn render(
    state: &mut BrowserWindow,
    theme: &ChromeTheme,
    _window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) -> Div {
    let (tabs, active_tab, drag_enabled) = {
        let shell = cx.global::<BrowserShell>();
        let session = lock(&shell.session);
        let Some(window_state) = session.window(state.window_id) else {
            return div();
        };
        (
            window_state.tabs.clone(),
            window_state.active_tab_id(),
            session.settings().flags.get(FeatureFlag::EnableDragTabs),
        )
    };

    div()
        .flex()
        .flex_row()
        .items_end()
        .h(px(TAB_STRIP_HEIGHT))
        .pl_1()
        .bg(theme.window_background)
        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
            handle_drag_move(this, event, drag_enabled, window, cx);
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
            if this.drag_press.take().is_some() {
                cx.notify();
            }
        }))
        .children(tabs.iter().map(|tab| {
            render_tab(state, tab, active_tab == Some(tab.id), drag_enabled, theme, cx)
        }))
        .child(
            div()
                .id("new-tab")
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h(px(26.0))
                .mb(px(4.0))
                .ml(px(2.0))
                .rounded_sm()
                .text_size(px(16.0))
                .text_color(theme.text_muted)
                .hover(|style| style.bg(theme.tab_hover_background))
                .child("+")
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, window, cx| {
                    this.new_tab(window, cx);
                })),
        )
}

fn render_tab(
    state: &BrowserWindow,
    tab: &Tab,
    is_active: bool,
    drag_enabled: bool,
    theme: &ChromeTheme,
    cx: &mut Context<BrowserWindow>,
) -> Div {
    let tab_id = tab.id;
    let title = tab.display_title().to_string();
    div()
        .id(SharedString::from(format!("tab-{tab_id}")))
        .flex()
        .flex_row()
        .items_center()
        .w(px(180.0))
        .h(px(30.0))
        .mr(px(2.0))
        .px_2()
        .rounded_t_sm()
        .text_size(px(12.5))
        .overflow_hidden()
        .when(is_active, |style| {
            style
                .bg(theme.tab_active_background)
                .text_color(theme.text)
        })
        .when(!is_active, |style| {
            style
                .bg(theme.tab_inactive_background)
                .text_color(theme.text_muted)
                .hover(|style| style.bg(theme.tab_hover_background))
        })
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            this.drag_press = drag_enabled.then_some((tab_id, event.position));
            this.run_command(BrowserCommand::ActivateTab { tab: tab_id }, window, cx);
        }))
        .child(
            div()
                .flex_1()
                .truncate()
                .child(SharedString::from(title)),
        )
        .child(
            div()
                .id(SharedString::from(format!("close-{tab_id}")))
                .flex()
                .items_center()
                .justify_center()
                .w(px(18.0))
                .h(px(18.0))
                .rounded_sm()
                .text_size(px(11.0))
                .hover(|style| style.bg(theme.tab_hover_background))
                .child("✕")
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, window, cx| {
                    this.run_command(BrowserCommand::CloseTab { tab: tab_id }, window, cx);
                })),
        )
        .when(state.drag_press.map(|(pressed, _)| pressed) == Some(tab_id), |style| {
            style.border_1().border_color(theme.accent)
        })
}

fn handle_drag_move(
    this: &mut BrowserWindow,
    event: &MouseMoveEvent,
    drag_enabled: bool,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    let Some((tab, origin)) = this.drag_press else {
        return;
    };
    if !drag_enabled {
        this.drag_press = None;
        return;
    }
    let distance = (event.position.x - origin.x).abs().0 + (event.position.y - origin.y).abs().0;
    if distance > DRAG_THRESHOLD {
        this.drag_press = None;
        this.run_command(BrowserCommand::DragOutTab { tab }, window, cx);
    }
}
