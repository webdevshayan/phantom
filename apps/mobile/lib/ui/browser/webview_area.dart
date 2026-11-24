import 'package:flutter/material.dart';

import '../../webview/webview_panel.dart';
import 'browser_controller.dart';

/// Binds a [WebViewPanel] to one slot of the browser controller and
/// forwards web view events into the controller.
class WebViewArea extends StatefulWidget {
  final BrowserController controller;
  final WebViewSlot slot;
  final String initialUrl;
  final bool adBlockEnabled;

  const WebViewArea({
    super.key,
    required this.controller,
    required this.slot,
    required this.initialUrl,
    required this.adBlockEnabled,
  });

  @override
  State<WebViewArea> createState() => _WebViewAreaState();
}

class _WebViewAreaState extends State<WebViewArea> {
  @override
  Widget build(BuildContext context) {
    return WebViewPanel(
      initialUrl: widget.initialUrl,
      adBlockEnabled: widget.adBlockEnabled,
      onHostReady: (host) => widget.controller.attachHost(widget.slot, host),
      onEvent: (event) =>
          widget.controller.handleWebViewEvent(widget.slot, event),
    );
  }

  @override
  void dispose() {
    widget.controller.detachHost(widget.slot);
    super.dispose();
  }
}
