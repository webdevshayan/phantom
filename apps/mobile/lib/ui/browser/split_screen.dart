import 'package:flutter/material.dart';

import 'browser_controller.dart';
import 'webview_area.dart';

/// Vertical split: primary pane on top, secondary pane below. Only mounted
/// when the split screen feature flag is on and the user opts in.
class SplitScreen extends StatelessWidget {
  final BrowserController controller;

  const SplitScreen({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: <Widget>[
        Expanded(
          child: WebViewArea(
            controller: controller,
            slot: WebViewSlot.primary,
            initialUrl: controller.activeTab.url,
            adBlockEnabled: controller.adBlockerEnabled,
          ),
        ),
        const SizedBox(
          width: double.infinity,
          height: 1,
          child: ColoredBox(color: Color(0x33888888)),
        ),
        Expanded(
          child: WebViewArea(
            controller: controller,
            slot: WebViewSlot.secondary,
            initialUrl: controller.splitTabUrl,
            adBlockEnabled: controller.adBlockerEnabled,
          ),
        ),
      ],
    );
  }
}
