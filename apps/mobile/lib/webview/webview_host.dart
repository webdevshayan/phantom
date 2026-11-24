import 'dart:async';

import 'package:flutter/foundation.dart';

enum WebViewEventKind {
  navigationStarted,
  pageLoaded,
  urlChanged,
  loadFailed,
  bridgeMessage
}

@immutable
class WebViewEvent {
  final WebViewEventKind kind;
  final String? url;
  final String? message;

  const WebViewEvent({required this.kind, this.url, this.message});
}

/// Contract for the embedded web view. The controller and the shell code
/// against this interface; the Flutter binding lives in webview_panel.dart
/// and tests use fakes.
abstract class WebViewHost {
  Stream<WebViewEvent> get events;

  Future<void> load(String url);
  Future<void> reload();
  Future<Map<String, String>> captureFormFields();
  Future<void> restoreFormFields(Map<String, String> fields);

  void dispose();
}
