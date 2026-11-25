//! Toolbar row: navigation buttons, the omnibox, and the 3-dot menu button.

use gpui::{div, prelude::*, px, Context, IntoElement, MouseButton, Styled, Window};

use crate::domain::command::BrowserCommand;
use crate::domain::tab::TabStatus;
use crate::ui::app::{lock, BrowserShell};
use crate::ui::menu;
use crate::ui::omnibox_view;
use crate::ui::theme::{ChromeTheme, TOOLBAR_HEIGHT};
use crate::ui::window::BrowserWindow;

pub fn render(
    state: &mut BrowserWindow,
    theme: &ChromeTheme,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) -> impl IntoElement {
    let (can_go_back, can_go_forward, is_loading) = {
        let shell = cx.global::<BrowserShell>();
        let session = lock(&shell.session);
        let Some(window_state) = session.window(state.window_id) else {
            return div();
        };
        match window_state.active_tab() {
            Some(tab) => (
                tab.can_go_back(),
                tab.can_go_forward(),
                tab.status == TabStatus::Loading,
            ),
            None => (false, false, false),
        }
    };

    let on_back = cx.listener(|this, _event, window, cx| {
        this.command_on_active_tab(|tab| BrowserCommand::GoBack { tab }, window, cx);
    });
    let on_forward = cx.listener(|this, _event, window, cx| {
        this.command_on_active_tab(|tab| BrowserCommand::GoForward { tab }, window, cx);
    });
    let on_reload_or_stop = cx.listener(move |this, _event, window, cx| {
        if is_loading {
            this.command_on_active_tab(|tab| BrowserCommand::StopLoading { tab }, window, cx);
        } else {
            this.command_on_active_tab(|tab| BrowserCommand::Reload { tab }, window, cx);
        }
    });
    let on_menu = cx.listener(|this, _event, _window, cx| {
        this.menu_open = !this.menu_open;
        cx.notify();
    });

    div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .h(px(TOOLBAR_HEIGHT))
        .px_1()
        .bg(theme.toolbar_background)
        .child(button(theme, "back", "←", can_go_back, on_back))
        .child(button(theme, "forward", "→", can_go_forward, on_forward))
        .child(button(
            theme,
            "reload",
            if is_loading { "✕" } else { "⟳" },
            true,
            on_reload_or_stop,
        ))
        .child(omnibox_view::render(state, theme, window, cx))
        .child(
            div()
                .id("chrome-menu-button")
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h(px(28.0))
                .mr(px(2.0))
                .rounded(px(6.0))
                .text_size(px(14.0))
                .text_color(theme.text_muted)
                .hover(|style| style.bg(theme.tab_hover_background))
                .child("⋮")
                .on_mouse_down(MouseButton::Left, on_menu),
        )
        .when(state.menu_open, |row| {
            row.child(menu::render(state, theme, cx))
        })
}

fn button(
    theme: &ChromeTheme,
    id: &'static str,
    glyph: &'static str,
    enabled: bool,
    on_press: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let base = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(28.0))
        .mx(px(1.0))
        .rounded(px(6.0))
        .text_size(px(14.0))
        .child(glyph);
    if enabled {
        base.text_color(theme.text)
            .hover(|style| style.bg(theme.tab_hover_background))
            .on_mouse_down(MouseButton::Left, on_press)
    } else {
        base.text_color(theme.text_muted)
    }
}
