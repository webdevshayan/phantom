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

  /// True when the embedded web view can step back in its own history.
  Future<bool> canGoBack();

  /// Steps the embedded web view one entry back in its own history. This
  /// replays the page the engine already holds, so POST results, scroll
  /// positions and page state survive; a fresh loadRequest cannot.
  Future<void> goBack();

  /// Steps the embedded web view one entry forward in its own history.
  Future<void> goForward();

  Future<Map<String, String>> captureFormFields();
  Future<void> restoreFormFields(Map<String, String> fields);

  void dispose();
}
