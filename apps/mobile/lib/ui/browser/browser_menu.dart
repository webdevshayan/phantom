import 'package:flutter/material.dart';

import '../devtools/devtools_overlay.dart';
import '../settings/settings_page.dart';
import 'browser_controller.dart';

/// The three-dot menu. Split view and the update check only appear when
/// their feature flags are on.
class BrowserMenu extends StatelessWidget {
  final BrowserController controller;

  const BrowserMenu({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return PopupMenuButton<String>(
      tooltip: 'More options',
      icon: const Icon(Icons.more_vert),
      onSelected: (value) => _onSelected(context, value),
      itemBuilder: (context) => <PopupMenuEntry<String>>[
        const PopupMenuItem<String>(
          value: 'new_tab',
          child: Text('New tab'),
        ),
        if (controller.preferences.flags.enableSplitScreen)
          PopupMenuItem<String>(
            value: 'toggle_split',
            child: Text(
                controller.splitViewActive ? 'Exit split view' : 'Split view'),
          ),
        const PopupMenuItem<String>(
          value: 'devtools',
          child: Text('Developer tools'),
        ),
        const PopupMenuItem<String>(
          value: 'settings',
          child: Text('Settings'),
        ),
        if (controller.preferences.flags.enableAutoUpdate)
          const PopupMenuItem<String>(
            value: 'check_updates',
            child: Text('Check for updates'),
          ),
      ],
    );
  }

  void _onSelected(BuildContext context, String value) {
    switch (value) {
      case 'new_tab':
        controller.addTab();
      case 'toggle_split':
        controller.toggleSplitView();
      case 'devtools':
        showDevToolsOverlay(context, controller.devToolsLogStore);
      case 'settings':
        Navigator.of(context).push(
          MaterialPageRoute<void>(
            builder: (_) => SettingsPage(controller: controller),
          ),
        );
      case 'check_updates':
        controller.navigate('https://github.com/webdevshayan/phantom/releases');
    }
  }
}
