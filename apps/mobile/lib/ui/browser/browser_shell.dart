import 'package:flutter/material.dart';

import 'browser_controller.dart';
import 'navigation_row.dart';
import 'omnibox_bar.dart';
import 'split_screen.dart';
import 'webview_area.dart';

/// Top-level chrome: omnibox on top, back/forward/reload, tab count and
/// the three-dot menu below, then the page area (single or split).
class BrowserShell extends StatefulWidget {
  final BrowserController controller;

  const BrowserShell({super.key, required this.controller});

  @override
  State<BrowserShell> createState() => _BrowserShellState();
}

class _BrowserShellState extends State<BrowserShell> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.controller.restoreSession();
    });
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final controller = widget.controller;
        final split = controller.splitViewActive;
        return Scaffold(
          body: SafeArea(
            child: Column(
              children: <Widget>[
                OmniboxBar(controller: controller),
                NavigationRow(controller: controller),
                Expanded(
                  child: split
                      ? SplitScreen(controller: controller)
                      : WebViewArea(
                          controller: controller,
                          slot: WebViewSlot.primary,
                          initialUrl: controller.activeTab.url,
                          adBlockEnabled: controller.adBlockerEnabled,
                        ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
