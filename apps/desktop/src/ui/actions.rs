//! Chrome actions and their key bindings. Bindings mirror a mainstream
//! browser so muscle memory carries over.

use gpui::{actions, KeyBinding};

actions!(
    phantom,
    [
        NewTab,
        CloseTab,
        NewWindow,
        FocusOmnibox,
        GoBack,
        GoForward,
        Reload,
        StopLoading,
        DevTools,
        OpenSettings,
        SplitHorizontal,
        SplitVertical,
        ClosePane,
        FocusNextPane,
        Quit,
    ]
);

/// Registers every chrome key binding. All bindings share the `Browser`
/// key context declared by the window's root element.
pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-t", NewTab, Some("Browser")),
        KeyBinding::new("ctrl-w", CloseTab, Some("Browser")),
        KeyBinding::new("ctrl-n", NewWindow, Some("Browser")),
        KeyBinding::new("ctrl-l", FocusOmnibox, Some("Browser")),
        KeyBinding::new("alt-left", GoBack, Some("Browser")),
        KeyBinding::new("alt-right", GoForward, Some("Browser")),
        KeyBinding::new("ctrl-r", Reload, Some("Browser")),
        KeyBinding::new("f5", Reload, Some("Browser")),
        KeyBinding::new("escape", StopLoading, Some("Browser")),
        KeyBinding::new("f12", DevTools, Some("Browser")),
        KeyBinding::new("ctrl-,", OpenSettings, Some("Browser")),
        KeyBinding::new("ctrl-shift-h", SplitHorizontal, Some("Browser")),
        KeyBinding::new("ctrl-shift-j", SplitVertical, Some("Browser")),
        KeyBinding::new("ctrl-shift-w", ClosePane, Some("Browser")),
        KeyBinding::new("ctrl-tab", FocusNextPane, Some("Browser")),
        KeyBinding::new("ctrl-shift-q", Quit, Some("Browser")),
    ]);
}
