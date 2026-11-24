//! Native settings window: theme, search engine, and the feature-flag
//! switches. Changes flow through a command queue into the session and are
//! persisted immediately by the session's flush.

use std::sync::{Arc, Mutex};

use gpui::{
    div, prelude::*, px, Context, Div, FontWeight, MouseButton, MouseDownEvent, Render,
    SharedString, Styled, Window,
};

use crate::domain::command::BrowserCommand;
use crate::domain::search::SearchEngine;
use crate::domain::settings::{FeatureFlag, Settings, Theme};
use crate::ui::app::{lock, BrowserShell};
use crate::ui::theme::{self, ChromeTheme};

pub struct SettingsView {
    queue: Arc<Mutex<Vec<BrowserCommand>>>,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let shell = cx.global::<BrowserShell>();
        Self {
            queue: shell.settings_queue.clone(),
        }
    }

    fn push(&self, command: BrowserCommand, cx: &mut Context<Self>) {
        lock(&self.queue).push(command);
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings: Settings = {
            let shell = cx.global::<BrowserShell>();
            lock(&shell.session).settings().clone()
        };
        let theme = theme::resolve(settings.theme, theme::os_prefers_dark(window));

        let on_light = cx.listener(|this: &mut SettingsView, _e: &MouseDownEvent, _w, cx| {
            this.push(
                BrowserCommand::SetTheme {
                    theme: Theme::Light,
                },
                cx,
            );
        });
        let on_dark = cx.listener(|this: &mut SettingsView, _e: &MouseDownEvent, _w, cx| {
            this.push(BrowserCommand::SetTheme { theme: Theme::Dark }, cx);
        });
        let on_system = cx.listener(|this: &mut SettingsView, _e: &MouseDownEvent, _w, cx| {
            this.push(
                BrowserCommand::SetTheme {
                    theme: Theme::System,
                },
                cx,
            );
        });
        let mut engine_rows = div().flex().flex_col().gap(px(4.0));
        for engine in SearchEngine::ALL {
            let selected = settings.search_engine == *engine;
            let label = engine.label();
            engine_rows = engine_rows.child(engine_row(
                &theme,
                cx,
                move |this: &mut SettingsView, _e: &MouseDownEvent, _w, cx| {
                    this.push(BrowserCommand::SetSearchEngine { engine: *engine }, cx);
                },
                label,
                selected,
            ));
        }
        let mut flag_rows = div().flex().flex_col().gap(px(10.0));
        for flag in FeatureFlag::ALL {
            let enabled = settings.flags.get(*flag);
            let description = flag.description();
            flag_rows = flag_rows.child(flag_row(
                &theme,
                cx,
                move |this: &mut SettingsView, _e: &MouseDownEvent, _w, cx| {
                    this.push(
                        BrowserCommand::ToggleFlag {
                            flag: *flag,
                            enabled: !enabled,
                        },
                        cx,
                    );
                },
                description,
                enabled,
            ));
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.window_background)
            .text_color(theme.text)
            .p(px(24.0))
            .gap(px(20.0))
            .overflow_y_scroll()
            .child(header(&theme, "Settings"))
            .child(
                section(&theme, "Appearance")
                    .child(option_row(
                        &theme,
                        cx,
                        "theme-light",
                        "Light",
                        settings.theme == Theme::Light,
                        on_light,
                    ))
                    .child(option_row(
                        &theme,
                        cx,
                        "theme-dark",
                        "Dark",
                        settings.theme == Theme::Dark,
                        on_dark,
                    ))
                    .child(option_row(
                        &theme,
                        cx,
                        "theme-system",
                        "System",
                        settings.theme == Theme::System,
                        on_system,
                    )),
            )
            .child(section(&theme, "Search engine").child(engine_rows))
            .child(section(&theme, "Feature flags").child(flag_rows))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(format!(
                        "Schema version {} — flags ship disabled and turn on per switch",
                        settings.schema_version
                    ))),
            )
    }
}

fn header(theme: &ChromeTheme, title: &'static str) -> Div {
    div()
        .text_size(px(20.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text)
        .child(title)
}

fn section(theme: &ChromeTheme, title: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme.text_muted)
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(theme.toolbar_background),
        )
}

fn option_row(
    theme: &ChromeTheme,
    cx: &Context<SettingsView>,
    id: &'static str,
    label: &'static str,
    selected: bool,
    on_press: impl Fn(&mut SettingsView, &MouseDownEvent, &mut Window, &mut Context<SettingsView>)
        + 'static,
) -> Div {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px_2()
        .py(px(6.0))
        .rounded(px(6.0))
        .text_size(px(13.0))
        .when(selected, |style| style.bg(theme.menu_hover))
        .hover(|style| style.bg(theme.menu_hover))
        .on_mouse_down(MouseButton::Left, cx.listener(on_press))
        .child(radio_dot(theme, selected))
        .child(label)
}

fn engine_row(
    theme: &ChromeTheme,
    cx: &Context<SettingsView>,
    on_press: impl Fn(&mut SettingsView, &MouseDownEvent, &mut Window, &mut Context<SettingsView>)
        + 'static,
    label: &'static str,
    selected: bool,
) -> Div {
    div()
        .id(SharedString::from(format!("engine-{label}")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px_2()
        .py(px(4.0))
        .rounded(px(6.0))
        .text_size(px(13.0))
        .hover(|style| style.bg(theme.menu_hover))
        .on_mouse_down(MouseButton::Left, cx.listener(on_press))
        .child(radio_dot(theme, selected))
        .child(label)
}

fn flag_row(
    theme: &ChromeTheme,
    cx: &Context<SettingsView>,
    on_press: impl Fn(&mut SettingsView, &MouseDownEvent, &mut Window, &mut Context<SettingsView>)
        + 'static,
    description: &'static str,
    enabled: bool,
) -> Div {
    div()
        .id(SharedString::from(format!("flag-{description}")))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_2()
        .py(px(4.0))
        .rounded(px(6.0))
        .text_size(px(13.0))
        .hover(|style| style.bg(theme.menu_hover))
        .on_mouse_down(MouseButton::Left, cx.listener(on_press))
        .child(description)
        .child(toggle(theme, enabled))
}

fn radio_dot(theme: &ChromeTheme, selected: bool) -> Div {
    div()
        .w(px(12.0))
        .h(px(12.0))
        .rounded_full()
        .border_1()
        .border_color(theme.accent)
        .when(selected, |dot| dot.bg(theme.accent))
}

fn toggle(theme: &ChromeTheme, on: bool) -> Div {
    div()
        .flex()
        .items_center()
        .w(px(36.0))
        .h(px(20.0))
        .rounded_full()
        .px(px(2.0))
        .when(on, |pill| pill.bg(theme.toggle_on).justify_end())
        .when(!on, |pill| pill.bg(theme.toggle_off).justify_start())
        .child(
            div()
                .w(px(14.0))
                .h(px(14.0))
                .rounded_full()
                .bg(gpui::white()),
        )
}
