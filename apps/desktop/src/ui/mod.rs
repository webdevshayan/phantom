//! GPUI browser chrome: tab strip, omnibox toolbar, menus, settings window.

pub mod actions;
pub mod app;
pub mod menu;
pub mod omnibox_view;
pub mod settings_window;
pub mod tab_strip;
pub mod theme;
pub mod toolbar;
pub mod window;

pub use app::launch;
