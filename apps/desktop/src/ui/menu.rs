//! The 3-dot dropdown menu. Tiling entries gray out while `enable_tiling`
//! is off, mirroring the switches in the settings window.

use gpui::{div, prelude::*, px, Context, Div, MouseDownEvent, MouseButton, Styled};

use crate::browser::BrowserWindow;
use crate::domain::command::BrowserCommand;
use crate::domain::layout::SplitDirection;
use crate::domain::settings::FeatureFlag;
use crate::domain::space::SpaceId;
use crate::domain::tab::TabStatus;
use crate::ui::app::{lock, BrowserShell};
use crate::ui::theme::ChromeTheme;

pub fn render(state: &BrowserWindow, theme: &ChromeTheme, cx: &Context<BrowserWindow>) -> Div {
    let (tiling, active_loading) = {
        let shell = cx.global::<BrowserShell>();
        let session = lock(&shell.session);
        let loading = session
            .window(state.window_id)
            .and_then(|window| window.active_tab())
            .map(|tab| tab.status == TabStatus::Loading)
            .unwrap_or(false);
        (
            session.settings().flags.get(FeatureFlag::EnableTiling),
            loading,
        )
    };

    let close_menu = cx.listener(|this: &mut BrowserWindow, _event: &MouseDownEvent, _window, cx| {
        this.menu_open = false;
        cx.notify();
    });

    div()
        .id("chrome-menu")
        .absolute()
        .top_full()
        .right(px(4.0))
        .mt(px(4.0))
        .w(px(248.0))
        .flex()
        .flex_col()
        .py_1()
        .rounded(px(8.0))
        .shadow(md())
        .bg(theme.menu_background)
        .border_1()
        .border_color(theme.border)
        .on_mouse_down(MouseButton::Left, close_menu)
        .child(entry(
            theme,
            "new-tab",
            "New tab",
            "Ctrl+T",
            true,
            cx,
            |this, _event, window, cx| {
                this.menu_open = false;
                this.new_tab(window, cx);
            },
        ))
        .child(entry(
            theme,
            "new-window",
            "New window",
            "Ctrl+N",
            true,
            cx,
            |this, _event, window, cx| {
                this.menu_open = false;
                this.run_command(
                    BrowserCommand::NewWindow {
                        space: SpaceId::default_space(),
                    },
                    window,
                    cx,
                );
            },
        ))
        .divider(theme)
        .child(entry(
            theme,
            "split-h",
            "Split horizontally",
            "Ctrl+Shift+H",
            tiling,
            cx,
            |this, _event, window, cx| {
                this.menu_open = false;
                this.split(SplitDirection::Horizontal, window, cx);
            },
        ))
        .child(entry(
            theme,
            "split-v",
            "Split vertically",
            "Ctrl+Shift+J",
            tiling,
            cx,
            |this, _event, window, cx| {
                this.menu_open = false;
                this.split(SplitDirection::Vertical, window, cx);
            },
        ))
        .child(entry(
            theme,
            "close-pane",
            "Close pane",
            "Ctrl+Shift+W",
            tiling,
            cx,
            |this, _event, window, cx| {
                this.menu_open = false;
                if let Some(pane) = this.focused_pane_id(cx) {
                    this.run_command(BrowserCommand::ClosePane { pane }, window, cx);
                }
            },
        ))
        .divider(theme)
        .child(entry(
            theme,
            "devtools",
            "Developer tools",
            "F12",
            true,
            cx,
            |this, _event, window, cx| {
                this.menu_open = false;
                this.command_on_active_tab(
                    |tab| BrowserCommand::ToggleDevTools { tab },
                    window,
                    cx,
                );
            },
        ))
        .child(entry(
            theme,
            "settings",
            "Settings",
            "Ctrl+,",
            true,
            cx,
            |this, _event, _window, cx| {
                this.menu_open = false;
                this.open_settings(cx);
            },
        ))
        .child(entry(
            theme,
            "quit",
            "Quit Phantom",
            "Ctrl+Shift+Q",
            true,
            cx,
            |this, _event, _window, cx| {
                this.menu_open = false;
                this.quit(cx);
            },
        ))
        .when(active_loading, |menu| {
            menu.child(entry(
                theme,
                "stop-loading",
                "Stop loading",
                "Esc",
                true,
                cx,
                |this, _event, window, cx| {
                    this.menu_open = false;
                    this.command_on_active_tab(
                        |tab| BrowserCommand::StopLoading { tab },
                        window,
                        cx,
                    );
                },
            ))
        })
}

#[allow(clippy::too_many_arguments)]
fn entry(
    theme: &ChromeTheme,
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    enabled: bool,
    cx: &Context<BrowserWindow>,
    on_press: impl Fn(&mut BrowserWindow, &MouseDownEvent, &mut gpui::Window, &mut Context<BrowserWindow>)
    + 'static,
) -> Div {
    let base = div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_3()
        .py(px(6.0))
        .text_size(px(13.0))
        .text_color(theme.text)
        .child(label)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_muted)
                .child(shortcut),
        );
    if enabled {
        base.hover(|style| style.bg(theme.menu_hover))
            .on_mouse_down(MouseButton::Left, cx.listener(on_press))
    } else {
        base.opacity(0.4)
    }
}

fn divider(theme: &ChromeTheme) -> Div {
    div().h(px(1.0)).my_1().mx_2().bg(theme.border)
}

fn md() -> gpui::BoxShadow {
    gpui::BoxShadow {
        offset: gpui::point(px(0.0), px(4.0)),
        blur_radius: px(16.0),
        spread_radius: px(0.0),
        color: gpui::black().opacity(0.35),
    }
}
