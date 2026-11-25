//! Chrome color tokens. Light and Dark are fixed palettes; `System` resolves
//! against the OS color scheme reported by the platform.

use gpui::{rgb, Hsla, Window, WindowAppearance};

use crate::domain::settings::Theme;

/// Height of the tab strip row, logical pixels.
pub const TAB_STRIP_HEIGHT: f32 = 36.0;
/// Height of the toolbar row (omnibox + navigation buttons), logical pixels.
pub const TOOLBAR_HEIGHT: f32 = 44.0;
/// Total chrome height above the content area.
pub const CHROME_HEIGHT: f32 = TAB_STRIP_HEIGHT + TOOLBAR_HEIGHT;

#[derive(Clone, Copy, Debug)]
pub struct ChromeTheme {
    pub window_background: Hsla,
    pub toolbar_background: Hsla,
    pub tab_active_background: Hsla,
    pub tab_inactive_background: Hsla,
    pub tab_hover_background: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub border: Hsla,
    pub accent: Hsla,
    pub omnibox_background: Hsla,
    pub omnibox_text: Hsla,
    pub menu_background: Hsla,
    pub menu_hover: Hsla,
    pub toggle_on: Hsla,
    pub toggle_off: Hsla,
}

/// True when the platform reports a dark color scheme.
pub fn os_prefers_dark(window: &Window) -> bool {
    window.appearance() == WindowAppearance::Dark
}

/// Resolves the effective palette for a settings theme.
pub fn resolve(theme: Theme, os_dark: bool) -> ChromeTheme {
    match theme {
        Theme::Light => light(),
        Theme::Dark => dark(),
        Theme::System => {
            if os_dark {
                dark()
            } else {
                light()
            }
        }
    }
}

fn light() -> ChromeTheme {
    ChromeTheme {
        window_background: rgb(0xdee1e6).into(),
        toolbar_background: rgb(0xe8eaed).into(),
        tab_active_background: rgb(0xf1f3f4).into(),
        tab_inactive_background: rgb(0xcfd3d9).into(),
        tab_hover_background: rgb(0xdadde1).into(),
        text: rgb(0x202124).into(),
        text_muted: rgb(0x5f6368).into(),
        border: rgb(0xc4c7cc).into(),
        accent: rgb(0x1a73e8).into(),
        omnibox_background: rgb(0xf1f3f4).into(),
        omnibox_text: rgb(0x202124).into(),
        menu_background: rgb(0xffffff).into(),
        menu_hover: rgb(0xf1f3f4).into(),
        toggle_on: rgb(0x1a73e8).into(),
        toggle_off: rgb(0xc4c7cc).into(),
    }
}

fn dark() -> ChromeTheme {
    ChromeTheme {
        window_background: rgb(0x202124).into(),
        toolbar_background: rgb(0x292a2d).into(),
        tab_active_background: rgb(0x35363a).into(),
        tab_inactive_background: rgb(0x1b1c1e).into(),
        tab_hover_background: rgb(0x2f3033).into(),
        text: rgb(0xe8eaed).into(),
        text_muted: rgb(0x9aa0a6).into(),
        border: rgb(0x3c4043).into(),
        accent: rgb(0x8ab4f8).into(),
        omnibox_background: rgb(0x303134).into(),
        omnibox_text: rgb(0xe8eaed).into(),
        menu_background: rgb(0x292a2d).into(),
        menu_hover: rgb(0x3c4043).into(),
        toggle_on: rgb(0x8ab4f8).into(),
        toggle_off: rgb(0x3c4043).into(),
    }
}
