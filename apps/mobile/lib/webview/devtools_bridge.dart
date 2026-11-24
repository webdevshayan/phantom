/// Builds the JavaScript injected into every finished page load. It hooks
/// the console and the network primitives and streams entries to the
/// dedicated JavaScript channel, which feeds the developer tools overlay.
class DevToolsBridge {
  static const String channelName = 'PhantomDevTools';

  const DevToolsBridge._();

  static String build() => '''
(function () {
  if (window.__phantomDevToolsInstalled) { return; }
  window.__phantomDevToolsInstalled = true;
  var send = function (payload) {
    try { $channelName.postMessage(JSON.stringify(payload)); } catch (e) {}
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
''';
}
