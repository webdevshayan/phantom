//! The JavaScript injected into every page load. It carries the developer
//! tools hooks (console, fetch, XHR), the form draft collector and the
//! form draft restorer — semantics ported one-to-one from the old
//! bridges — plus the chrome overlay bootstrap. The transport changes
//! from the dedicated JavaScript channel to the phantom:// event
//! endpoints; the chrome itself rides on the same injection and talks to
//! the core exclusively through the binary state protocol.

/// Event endpoint kinds understood by the command dispatcher.
pub const DEVTOOLS_EVENT: &str = "devtools";
pub const FORM_EVENT: &str = "form";

const CHROME_CSS: &str = include_str!("../../ui/chrome.css");
const CHROME_JS: &str = include_str!("../../ui/chrome.js");

const SHELL_GUARD: &str = r#"
  if (window !== window.top) { return; }
  if (window.__phantomShellInstalled) { if (window.__phantomWake) { window.__phantomWake(); } return; }
  window.__phantomShellInstalled = true;
  var style = document.createElement('style');
  style.id = 'phantom-chrome-style';
  (document.head || document.documentElement).appendChild(style);
"#;

/// Hooks the console and the network primitives and streams entries to
/// the core. Ported from the old devtools bridge; the dedicated
/// JavaScript channel becomes a phantom:// event request.
const DEVTOOLS_HOOK: &str = r#"
  (function () {
    if (window.__phantomDevToolsInstalled) { return; }
    window.__phantomDevToolsInstalled = true;
    var send = function (payload) {
      try { window.__phantomSendEvent('devtools', JSON.stringify(payload)); } catch (e) {}
    };
    var wrap = function (level) {
      var original = console[level] ? console[level].bind(console) : function () {};
      console[level] = function () {
        var parts = [];
        for (var i = 0; i < arguments.length; i++) {
          try {
            parts.push(typeof arguments[i] === 'string' ? arguments[i] : JSON.stringify(arguments[i]));
          } catch (e) {
            parts.push(String(arguments[i]));
          }
        }
        send({ kind: 'console', level: level, message: parts.join(' '), at: Date.now() });
        original.apply(null, arguments);
      };
    };
    ['log', 'info', 'warn', 'error'].forEach(wrap);
    window.addEventListener('error', function (event) {
      send({ kind: 'console', level: 'error', message: String(event.message), at: Date.now() });
    });
    window.addEventListener('unhandledrejection', function (event) {
      send({ kind: 'console', level: 'error', message: 'Unhandled rejection: ' + String(event.reason), at: Date.now() });
    });
    if (window.fetch && !window.fetch.__phantomPatched) {
      var originalFetch = window.fetch;
      var wrappedFetch = function () {
        var input = arguments[0];
        var url = typeof input === 'string' ? input : (input && input.url) || '';
        var method = (arguments[1] && arguments[1].method) || 'GET';
        var started = Date.now();
        return originalFetch.apply(null, arguments).then(function (response) {
          send({ kind: 'network', method: method, url: String(url), status: response.status, durationMs: Date.now() - started, at: started });
          return response;
        }, function (failure) {
          send({ kind: 'network', method: method, url: String(url), status: 0, durationMs: Date.now() - started, at: started });
          throw failure;
        });
      };
      wrappedFetch.__phantomPatched = true;
      window.fetch = wrappedFetch;
    }
    if (window.XMLHttpRequest && !window.XMLHttpRequest.prototype.__phantomPatched) {
      var proto = window.XMLHttpRequest.prototype;
      var originalOpen = proto.open;
      proto.open = function (method, url) {
        this.__phantomMethod = method;
        this.__phantomUrl = url;
        this.__phantomStarted = Date.now();
        return originalOpen.apply(this, arguments);
      };
      var originalSend = proto.send;
      proto.send = function () {
        var xhr = this;
        xhr.addEventListener('loadend', function () {
          send({
            kind: 'network',
            method: xhr.__phantomMethod || 'GET',
            url: String(xhr.__phantomUrl || ''),
            status: xhr.status,
            durationMs: Date.now() - (xhr.__phantomStarted || Date.now()),
            at: xhr.__phantomStarted || Date.now()
          });
        });
        return originalSend.apply(this, arguments);
      };
      proto.__phantomPatched = true;
    }
  })();
"#;

/// The form draft collector plus a debounce listener that streams drafts
/// while the user types. The collector excludes secret input types here
/// and again on the core side, so saved drafts only ever contain safe
/// field types. Ported from the old capture bridge; streaming as the user
/// types keeps the draft alive even if the process dies mid-form.
const FORM_LISTENER: &str = r#"
  (function () {
    if (window.__phantomFormListenerInstalled) { return; }
    window.__phantomFormListenerInstalled = true;
    window.__phantomCollectForms = function () {
      var fields = {};
      var nodes = document.querySelectorAll('input, textarea');
      for (var i = 0; i < nodes.length; i++) {
        var node = nodes[i];
        var type = (node.getAttribute('type') || 'text').toLowerCase();
        if (type === 'password' || type === 'hidden' || type === 'file' || type === 'checkbox' || type === 'radio') { continue; }
        if (!node.value) { continue; }
        var key = node.name || node.id || ('field-' + i);
        fields[key] = String(node.value);
      }
      return JSON.stringify({ url: location.href, fields: fields });
    };
    var timer = null;
    var flush = function () {
      timer = null;
      try { window.__phantomSendEvent('form', window.__phantomCollectForms()); } catch (e) {}
    };
    var schedule = function () {
      if (timer === null) { timer = setTimeout(flush, 400); }
    };
    document.addEventListener('input', schedule, true);
    document.addEventListener('change', schedule, true);
  })();
"#;

/// Assembles the full initialization script: shell guard, chrome
/// stylesheet, chrome renderer, devtools hooks and form listener.
pub fn initialization_script() -> String {
    let css_json = serde_json::to_string(CHROME_CSS).expect("css escapes as a json string");
    let mut script = String::with_capacity(CHROME_JS.len() + CHROME_CSS.len() + 4096);
    script.push_str("(() => {\n");
    script.push_str(SHELL_GUARD);
    script.push_str("\n  style.textContent = ");
    script.push_str(&css_json);
    script.push_str(";\n");
    script.push_str(CHROME_JS);
    script.push_str("\n");
    script.push_str(DEVTOOLS_HOOK);
    script.push_str("\n");
    script.push_str(FORM_LISTENER);
    script.push_str("\n})();\n");
    script
}

/// Builds the script that re-applies saved form drafts after a page
/// load. Values are only applied to matching, safe field types on the
/// page that saved them. Ported from the old restore bridge.
pub fn form_restore_script(fields: &[(String, String)]) -> String {
    let mut map = serde_json::Map::new();
    for (key, value) in fields {
        map.insert(key.clone(), serde_json::Value::String(value.clone()));
    }
    let encoded = serde_json::Value::Object(map).to_string();
    format!(
        "(function () {{\n  var values = {encoded};\n  var nodes = document.querySelectorAll('input, textarea');\n  for (var i = 0; i < nodes.length; i++) {{\n    var node = nodes[i];\n    var type = (node.getAttribute('type') || 'text').toLowerCase();\n    if (type === 'password' || type === 'hidden' || type === 'file' || type === 'checkbox' || type === 'radio') {{ continue; }}\n    var key = node.name || node.id || ('field-' + i);\n    if (Object.prototype.hasOwnProperty.call(values, key)) {{\n      node.value = String(values[key]);\n    }}\n  }}\n}})();"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFE_EXCLUSION: &str =
        "type === 'password' || type === 'hidden' || type === 'file' || type === 'checkbox' || type === 'radio'";

    #[test]
    fn the_shell_installs_once_and_defers_to_the_top_frame() {
        let script = initialization_script();
        assert!(script.contains("window.__phantomShellInstalled"), "install guard");
        assert!(script.contains("window !== window.top"), "top-frame guard");
        assert!(script.contains("window.__phantomWake"), "wake bridge");
    }

    #[test]
    fn the_shell_embeds_the_chrome_assets() {
        let script = initialization_script();
        assert!(script.contains("#phantom-chrome"), "chrome css embedded");
        assert!(script.contains("__pcDecode"), "chrome decoder embedded");
        assert!(script.contains("phantom://"), "state endpoint referenced");
    }

    #[test]
    fn the_devtools_hook_streams_console_and_network() {
        let script = initialization_script();
        assert!(script.contains("__phantomDevToolsInstalled"));
        assert!(script.contains("'console'"));
        assert!(script.contains("'network'"));
        assert!(script.contains("window.__phantomSendEvent('devtools'"));
    }

    #[test]
    fn secret_input_types_are_never_captured() {
        let script = initialization_script();
        assert_eq!(
            script.matches(SAFE_EXCLUSION).count(),
            1,
            "the collector excludes secrets"
        );
        let restore = form_restore_script(&[("user".to_string(), "ada".to_string())]);
        assert!(restore.contains(SAFE_EXCLUSION), "restore keeps the exclusion");
        assert!(script.contains("__phantomCollectForms"));
    }

    #[test]
    fn the_state_channel_never_carries_json() {
        // The chrome decodes bincode snapshots; JSON only ever appears in
        // the page instrumentation streams (devtools, form drafts).
        assert!(!CHROME_JS.contains("JSON.parse"));
        assert!(!CHROME_JS.contains("invoke("), "no platform bridge dependency");
    }

    #[test]
    fn restore_applies_only_safe_matching_fields() {
        let script = form_restore_script(&[
            ("user".to_string(), "ada".to_string()),
            ("note".to_string(), "hello world".to_string()),
        ]);
        assert!(script.contains(SAFE_EXCLUSION), "restore keeps the exclusion");
        assert!(script.contains(r#""user":"ada""#), "values embedded");
        assert!(script.contains(r#""note":"hello world""#));
        assert!(script.contains("hasOwnProperty"), "matching stays exact");
        let quoted = form_restore_script(&[("a\"b".to_string(), "c\"d".to_string())]);
        assert!(quoted.contains(r#""a\"b":"c\"d""#), "values are json-escaped");
    }
}
