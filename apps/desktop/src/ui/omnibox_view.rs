//! The omnibox: url/search input built on the domain caret buffer.

use gpui::{
    div, prelude::*, px, Context, Div, KeyDownEvent, MouseButton, Styled, Window,
};

use crate::browser::BrowserWindow;
use crate::domain::command::{BrowserCommand, NavigationTarget};
use crate::domain::omnibox::{classify, OmniboxInput};
use crate::ui::app::{lock, BrowserShell};
use crate::ui::theme::ChromeTheme;

/// Renders the omnibox row segment. Focus follows the omnibox focus handle;
/// the buffer stays in sync with the active tab's url while not edited.
pub fn render(
    state: &mut BrowserWindow,
    theme: &ChromeTheme,
    _window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) -> Div {
    div()
        .id("omnibox")
        .track_focus(&state.omnibox_focus)
        .key_context("Omnibox")
        .flex()
        .flex_row()
        .items_center()
        .flex_1()
        .h(px(30.0))
        .px_2()
        .mx_2()
        .rounded_sm()
        .bg(theme.omnibox_background)
        .text_color(theme.omnibox_text)
        .text_size(px(13.0))
        .overflow_hidden()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, window, cx| {
            if !this.omnibox.is_focused() {
                this.omnibox.focus(false);
                window.focus(&this.omnibox_focus);
                cx.notify();
            }
        }))
        .on_key_down(cx.listener(handle_omnibox_key))
        .child(render_caret_text(state, theme))
}

fn render_caret_text(state: &BrowserWindow, theme: &ChromeTheme) -> gpui::AnyElement {
    let text = state.omnibox.text().to_string();
    let display = if text.is_empty() && !state.omnibox.is_focused() {
        String::new()
    } else {
        text
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .overflow_hidden()
        .child(
            div()
                .truncate()
                .text_color(if state.omnibox.is_focused() {
                    theme.omnibox_text
                } else {
                    theme.text_muted
                })
                .child(SharedString::from(display)),
        )
}

/// Single-line editor behavior on top of `OmniboxBuffer`: printable keys,
/// caret motion, backspace/delete, paste via clipboard on ctrl-v, escape to
/// restore, enter to commit (url or engine search).
fn handle_omnibox_key(
    this: &mut BrowserWindow,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    let key = event.keystroke.key.clone();
    let modifiers = event.keystroke.modifiers;

    if key == "enter" {
        commit(this, window, cx);
        cx.stop_propagation();
        return;
    }
    if key == "escape" {
        let url = active_tab_url(this, cx);
        this.omnibox.show_url(url.as_deref());
        this.omnibox.blur();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "backspace" {
        this.omnibox.backspace();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "delete" {
        this.omnibox.delete();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "left" {
        this.omnibox.move_left();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "right" {
        this.omnibox.move_right();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "home" {
        this.omnibox.move_home();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "end" {
        this.omnibox.move_end();
        cx.notify();
        cx.stop_propagation();
        return;
    }
    // printable characters: plain or shift-modified single-character keys
    let control_held = modifiers.control || modifiers.alt || modifiers.platform || modifiers.function;
    if !control_held && key.chars().count() == 1 {
        this.omnibox.insert(&key);
        cx.notify();
        cx.stop_propagation();
    }
}

fn commit(this: &mut BrowserWindow, window: &mut Window, cx: &mut Context<BrowserWindow>) {
    let input = this.omnibox.text().to_string();
    this.omnibox.blur();
    let engine = {
        let shell = cx.global::<BrowserShell>();
        lock(&shell.session).settings().search_engine
    };
    let target = match classify(&input) {
        OmniboxInput::Url(url) => NavigationTarget::Url(url),
        OmniboxInput::Search(query) => NavigationTarget::Search { query, engine },
    };
    let Some(tab) = this.active_tab_id(cx) else {
        return;
    };
    this.run_command(
        BrowserCommand::Navigate { tab, target },
        window,
        cx,
    );
}

fn active_tab_url(this: &BrowserWindow, cx: &Context<BrowserWindow>) -> Option<String> {
    let shell = cx.global::<BrowserShell>();
    let session = lock(&shell.session);
    session
        .window(this.window_id)
        .and_then(|window| window.active_tab())
        .and_then(|tab| tab.url.as_ref().map(|url| url.to_string()))
}
