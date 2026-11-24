//! Injected into every webview. Reports navigation state to the host over
//! the validated IPC channel. The bridge runs in page context, so the host
//! treats everything it receives as untrusted (see `infra::ipc`).

pub const STATE_BRIDGE_JS: &str = r#"(function () {
  'use strict';
  function report() {
    try {
      var nav = {
        url: String((location && location.href) || ''),
        title: String((document && document.title) || ''),
        can_go_back: !!(history && history.length > 1),
        can_go_forward: false,
        is_loading: !!(document && document.readyState === 'loading')
      };
      window.ipc.postMessage(JSON.stringify({ type: 'navigation_state_changed', state: nav }));
    } catch (e) { /* the host validates everything anyway */ }
  }
  function openRequested(url, background) {
    try {
      window.ipc.postMessage(JSON.stringify({ type: 'open_in_new_tab', url: String(url || ''), background: !!background }));
    } catch (e) { /* ignored */ }
  }
  ['DOMContentLoaded', 'load', 'pageshow'].forEach(function (name) {
    window.addEventListener(name, report, true);
  });
  document.addEventListener('readystatechange', report);
  window.addEventListener('popstate', report);
  var pushState = history.pushState;
  if (pushState) {
    history.pushState = function () {
      var result = pushState.apply(this, arguments);
      report();
      return result;
    };
  }
  // target=_blank links and window.open surface through the validated channel
  document.addEventListener('click', function (event) {
    var anchor = event.target && event.target.closest && event.target.closest('a[target="_blank"]');
    if (anchor && anchor.href) {
      event.preventDefault();
      openRequested(anchor.href, false);
    }
  }, true);
  var nativeOpen = window.open;
  window.open = function (url) {
    openRequested(url, false);
    return null;
  };
  // slow-loading pages update late; a light poll keeps the strip honest
  setInterval(report, 1000);
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_posts_only_contracted_message_types() {
        assert!(STATE_BRIDGE_JS.contains("navigation_state_changed"));
        assert!(STATE_BRIDGE_JS.contains("open_in_new_tab"));
        assert!(!STATE_BRIDGE_JS.contains("eval("));
        assert!(!STATE_BRIDGE_JS.contains("http://"));
        assert!(!STATE_BRIDGE_JS.contains("https://"));
    }

    #[test]
    fn bridge_stays_small_enough_to_inject_everywhere() {
        // it ships in every webview of every space; keep it lean
        assert!(
            STATE_BRIDGE_JS.len() < 4 * 1024,
            "{} bytes",
            STATE_BRIDGE_JS.len()
        );
    }
}
