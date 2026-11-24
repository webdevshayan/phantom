//! Phantom desktop shell.
//!
//! The crate is split along the monorepo convention:
//! - `domain`  — pure browser model: tabs, spaces, commands, settings, layout
//! - `infra`   — persistence (SQLite) and the strict host-to-webview IPC contract
//! - `webview` — wry-backed webview hosting with per-space data isolation
//! - `ui`      — GPUI browser chrome (tab strip, omnibox, menus, settings)
//! - `browser` — session controller wiring domain + infra together
//!
//! The UI and webview layers are feature-gated (`ui-shell`, `webview-host`)
//! so the model and persistence layers build and test on a bare toolchain.

pub mod domain;
