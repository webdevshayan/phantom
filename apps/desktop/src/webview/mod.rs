//! wry-backed webview hosting with per-space data isolation.

pub mod bridge;
pub mod internal_pages;

#[cfg(feature = "webview-host")]
pub mod host;
