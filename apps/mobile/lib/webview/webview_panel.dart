import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';

import '../../domain/ad_blocker.dart';
import 'devtools_bridge.dart';
import 'flutter_webview_host.dart';
import 'webview_host.dart';

/// Renders one web view pane and hands its host to the owner (the browser
/// shell) so navigation commands can reach it. The ad blocker flag is read
/// live from the widget, so toggling the flag re-gates navigation without
/// rebuilding the underlying platform view.
class WebViewPanel extends StatefulWidget {
  final String initialUrl;
  final bool adBlockEnabled;
  final void Function(WebViewHost host) onHostReady;
  final void Function(WebViewEvent event) onEvent;

  const WebViewPanel({
    super.key,
    required this.initialUrl,
    required this.adBlockEnabled,
    required this.onHostReady,
    required this.onEvent,
  });

  @override
  State<WebViewPanel> createState() => _WebViewPanelState();
}

class _WebViewPanelState extends State<WebViewPanel> {
  late final WebViewController _controller;
  late final PanelWebViewHost _host;

  @override
  void initState() {
    super.initState();
    _controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..addJavaScriptChannel(
        DevToolsBridge.channelName,
        onMessageReceived: (JavaScriptMessage message) {
          widget.onEvent(WebViewEvent(
            kind: WebViewEventKind.bridgeMessage,
            message: message.message,
          ));
        },
      )
      ..setNavigationDelegate(
        NavigationDelegate(
          onNavigationRequest: (NavigationRequest request) {
            if (widget.adBlockEnabled && AdBlockRules.blocksUrl(request.url)) {
              return NavigationDecision.prevent;
            }
            return NavigationDecision.navigate;
          },
          onPageStarted: (String url) {
            widget.onEvent(WebViewEvent(
              kind: WebViewEventKind.navigationStarted,
              url: url,
            ));
          },
          onPageFinished: (String url) async {
            widget.onEvent(WebViewEvent(
              kind: WebViewEventKind.pageLoaded,
              url: url,
            ));
            try {
              await _controller.runJavaScript(DevToolsBridge.build());
            } on Exception {
              // Injection failures must never break the page.
            }
          },
          onWebResourceError: (WebResourceError error) {
            widget.onEvent(WebViewEvent(
              kind: WebViewEventKind.loadFailed,
              message: error.description,
            ));
          },
        ),
      );
    _host = PanelWebViewHost(_controller);
    widget.onHostReady(_host);
    final initial = Uri.tryParse(widget.initialUrl);
    if (initial != null && initial.hasScheme && initial.scheme != 'about') {
      _controller.loadRequest(initial);
    }
  }

  @override
  Widget build(BuildContext context) {
    return WebViewWidget(controller: _controller);
  }
}
