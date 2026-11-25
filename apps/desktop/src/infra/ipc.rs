//! The only door between web content and the browser core.
//!
//! Webviews post small JSON messages; everything else is refused. The
//! validator is deliberately paranoid:
//! - hard byte cap before parsing
//! - tagged enums with `deny_unknown_fields` everywhere
//! - scheme allowlist on every url field
//! - length caps and control-character rejection on text fields
//! - errors are static strings: no payload fragments ever flow back out

use serde::{Deserialize, Serialize};

use crate::domain::command::{validate_navigation, validate_page_initiated};
use crate::domain::settings::Theme;

/// Hard cap on any single IPC message, applied before parsing.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_URL_BYTES: usize = 2048;
pub const MAX_TITLE_BYTES: usize = 1024;

/// Messages a webview may send to the host. Anything outside this set is
/// rejected before it can influence session state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WebviewToHost {
    NavigationStateChanged { state: NavigationState },
    OpenInNewTab { url: String, background: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationState {
    pub url: String,
    pub title: String,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub is_loading: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IpcError {
    #[error("message exceeds the size limit")]
    TooLarge,
    #[error("message is not valid JSON")]
    Malformed,
    #[error("message uses an unknown type or field")]
    UnknownType,
    #[error("message field is out of range")]
    FieldOutOfRange,
    #[error("navigation target is not permitted")]
    NavigationNotAllowed,
}

/// Validates and decodes one raw message posted by a webview.
pub fn decode_webview_message(raw: &str) -> Result<WebviewToHost, IpcError> {
    if raw.len() > MAX_MESSAGE_BYTES {
        return Err(IpcError::TooLarge);
    }
    let message: WebviewToHost = serde_json::from_str(raw).map_err(|error| {
        let text = error.to_string();
        if text.contains("unknown field") || text.contains("unknown variant") {
            IpcError::UnknownType
        } else {
            IpcError::Malformed
        }
    })?;
    validate(&message)?;
    Ok(message)
}

fn validate(message: &WebviewToHost) -> Result<(), IpcError> {
    match message {
        WebviewToHost::NavigationStateChanged { state } => {
            // a committed-location report from the top frame; the full
            // allowlist applies so a typed `file://` page can still report
            // its real location back to the omnibox
            validate_url_text(&state.url, true, validate_navigation)?;
            validate_title(&state.title)
        }
        WebviewToHost::OpenInNewTab { url, .. } => {
            // the page asked for an open: `file` and every other
            // chrome-reserved scheme is refused before it reaches the model
            validate_url_text(url, false, validate_page_initiated)
        }
    }
}

/// Url fields must parse and pass the supplied scheme check.
/// `NavigationState.url` may be empty (fresh webview); an open request must
/// carry a real target.
fn validate_url_text(
    raw: &str,
    allow_empty: bool,
    check: fn(&url::Url) -> Result<(), crate::domain::command::NavigationError>,
) -> Result<(), IpcError> {
    if raw.len() > MAX_URL_BYTES {
        return Err(IpcError::FieldOutOfRange);
    }
    if raw.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(IpcError::NavigationNotAllowed)
        };
    }
    let url = url::Url::parse(raw).map_err(|_| IpcError::NavigationNotAllowed)?;
    check(&url).map_err(|_| IpcError::NavigationNotAllowed)
}

fn validate_title(title: &str) -> Result<(), IpcError> {
    if title.len() > MAX_TITLE_BYTES {
        return Err(IpcError::FieldOutOfRange);
    }
    if title.chars().any(char::is_control) {
        return Err(IpcError::FieldOutOfRange);
    }
    Ok(())
}

/// Host-to-webview updates. Trusted direction; kept tiny on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostToWebview {
    ThemeChanged { theme: Theme },
}

pub fn encode_host_message(message: &HostToWebview) -> String {
    // infallible for this enum; the fallback keeps the channel honest anyway
    serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_json(url: &str, title: &str) -> String {
        format!(
            "{{\"type\":\"navigation_state_changed\",\"state\":{{\"url\":\"{url}\",\"title\":\"{title}\",\"can_go_back\":false,\"can_go_forward\":false,\"is_loading\":false}}}}"
        )
    }

    #[test]
    fn valid_navigation_state_decodes() {
        let decoded = decode_webview_message(&state_json("https://example.com/", "Example"))
            .expect("valid message");
        match decoded {
            WebviewToHost::NavigationStateChanged { state } => {
                assert_eq!(state.url, "https://example.com/");
                assert_eq!(state.title, "Example");
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn empty_url_is_allowed_only_for_state_reports() {
        let state = decode_webview_message(&state_json("", ""));
        assert!(state.is_ok());
        let open = decode_webview_message(
            "{\"type\":\"open_in_new_tab\",\"url\":\"\",\"background\":true}",
        );
        assert_eq!(open, Err(IpcError::NavigationNotAllowed));
    }

    #[test]
    fn oversize_messages_are_dropped_before_parsing() {
        let raw = "x".repeat(MAX_MESSAGE_BYTES + 1);
        assert_eq!(decode_webview_message(&raw), Err(IpcError::TooLarge));
    }

    #[test]
    fn unknown_type_and_fields_are_refused() {
        assert_eq!(
            decode_webview_message("{\"type\":\"exfiltrate\",\"url\":\"https://x\"}"),
            Err(IpcError::UnknownType)
        );
        assert_eq!(
            decode_webview_message(
                &state_json("https://a.b/", "t")
                    .replace("\"title\":\"t\"", "\"title\":\"t\",\"evil\":1")
            ),
            Err(IpcError::UnknownType)
        );
        assert_eq!(decode_webview_message("[]"), Err(IpcError::Malformed));
        assert_eq!(decode_webview_message("{oops"), Err(IpcError::Malformed));
    }

    #[test]
    fn unsafe_schemes_are_refused_on_open_requests() {
        for bad in [
            "javascript:alert(1)",
            "data:text/html,<b>",
            "vbscript:x",
            "ftp://f",
            // file targets stay reserved for typed omnibox input: a page
            // must not be able to hand the shell a local path
            "file:///tmp/payload",
            "file://localhost/C:/Users",
        ] {
            let raw =
                format!("{{\"type\":\"open_in_new_tab\",\"url\":\"{bad}\",\"background\":false}}");
            assert_eq!(
                decode_webview_message(&raw),
                Err(IpcError::NavigationNotAllowed),
                "{bad}"
            );
        }
    }

    #[test]
    fn state_reports_accept_typed_file_pages_but_opens_do_not() {
        // the top frame of a page the user actually opened by typing may
        // report its location; the same url through an open request is refused
        let state = state_json("file:///home/user/page.html", "Local");
        assert!(decode_webview_message(&state).is_ok());
        let open = decode_webview_message(
            "{\"type\":\"open_in_new_tab\",\"url\":\"file:///home/user/page.html\",\"background\":false}",
        );
        assert_eq!(open, Err(IpcError::NavigationNotAllowed));
    }

    #[test]
    fn titles_are_length_and_control_checked() {
        let long = "t".repeat(MAX_TITLE_BYTES + 1);
        assert_eq!(
            decode_webview_message(&state_json("https://a.b/", &long)),
            Err(IpcError::FieldOutOfRange)
        );
        // serialize through serde so the newline is JSON-escaped exactly the
        // way the real bridge would emit it
        let state = NavigationState {
            url: "https://a.b/".into(),
            title: "bad\nnewlines".into(),
            can_go_back: false,
            can_go_forward: false,
            is_loading: false,
        };
        let raw = serde_json::to_string(&WebviewToHost::NavigationStateChanged { state }).unwrap();
        assert_eq!(decode_webview_message(&raw), Err(IpcError::FieldOutOfRange));
    }

    #[test]
    fn error_text_never_echoes_payload() {
        let raw = "{\"type\":\"open_in_new_tab\",\"url\":\"javascript:top-secret-payload\",\"background\":false}";
        if let Err(error) = decode_webview_message(raw) {
            let text = error.to_string();
            assert!(!text.contains("javascript"), "{text}");
            assert!(!text.contains("payload"), "{text}");
        } else {
            panic!("expected rejection");
        }
    }

    #[test]
    fn host_encoding_is_stable_json() {
        let encoded = encode_host_message(&HostToWebview::ThemeChanged { theme: Theme::Dark });
        assert_eq!(encoded, "{\"type\":\"theme_changed\",\"theme\":\"dark\"}");
    }
}
