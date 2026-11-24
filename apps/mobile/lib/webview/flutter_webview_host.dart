import 'dart:async';
import 'dart:convert';

import 'package:webview_flutter/webview_flutter.dart';

import 'form_capture_bridge.dart';
import 'form_restore_bridge.dart';
import 'webview_host.dart';

/// [WebViewHost] backed by the webview_flutter controller.
class PanelWebViewHost implements WebViewHost {
  final WebViewController _controller;

  PanelWebViewHost(this._controller);

  @override
  Stream<WebViewEvent> get events => const Stream<WebViewEvent>.empty();

  @override
  Future<void> load(String url) async {
    final parsed = Uri.tryParse(url);
    if (parsed == null || !parsed.hasScheme) {
      return;
    }
    try {
      await _controller.loadRequest(parsed);
    } on Exception {
      // Bad targets are ignored; the page keeps whatever loaded last.
    }
  }

  @override
  Future<void> reload() async {
    try {
      await _controller.reload();
    } on Exception {
      // Reload failures surface through web resource error events.
    }
  }

  @override
  Future<Map<String, String>> captureFormFields() async {
    final raw = await _runScript(FormCaptureBridge.build());
    if (raw.isEmpty) {
      return <String, String>{};
    }
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        return decoded.map(
          (key, value) => MapEntry(key.toString(), value?.toString() ?? ''),
        );
      }
    } on FormatException {
      // Unparseable results are treated as no captured fields.
    }
    return <String, String>{};
  }

  @override
  Future<void> restoreFormFields(Map<String, String> fields) async {
    if (fields.isEmpty) {
      return;
    }
    try {
      await _controller.runJavaScript(FormRestoreBridge.build(fields));
    } on Exception {
      // Restore is best effort.
    }
  }

  Future<String> _runScript(String script) async {
    try {
      final result = await _controller.runJavaScriptReturningResult(script);
      var raw = result is String ? result : result.toString();
      if (raw.length >= 2 && raw.startsWith('"') && raw.endsWith('"')) {
        final unquoted = jsonDecode(raw);
        if (unquoted is String) {
          raw = unquoted;
        }
      }
      return raw;
    } catch (_) {
      return '';
    }
  }

  @override
  void dispose() {}
}
