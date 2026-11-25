import 'package:flutter/material.dart';

import '../../domain/preferences.dart';
import '../../domain/search_engine.dart';
import '../browser/browser_controller.dart';

/// The settings page, deliberately limited to three sections: theme,
/// search engine and the feature flag switches.
class SettingsPage extends StatelessWidget {
  final BrowserController controller;

  const SettingsPage({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: AnimatedBuilder(
        animation: controller,
        builder: (context, _) {
          final preferences = controller.preferences;
          return ListView(
            children: <Widget>[
              const _SectionHeader('Theme'),
              for (final preference in ThemePreference.values)
                RadioListTile<ThemePreference>(
                  value: preference,
                  groupValue: preferences.theme,
                  title: Text(preference.label),
                  onChanged: (value) {
                    if (value != null) {
                      controller.setTheme(value);
                    }
                  },
                ),
              const Divider(),
              const _SectionHeader('Search engine'),
              for (final engine in SearchEngine.catalog)
                RadioListTile<SearchEngineId>(
                  value: engine.id,
                  groupValue: preferences.searchEngine,
                  title: Text(engine.label),
                  onChanged: (value) {
                    if (value != null) {
                      controller.setSearchEngine(value);
                    }
                  },
                ),
              const Divider(),
              const _SectionHeader('Feature flags'),
              SwitchListTile(
                value: preferences.flags.enableSplitScreen,
                title: const Text('Split screen'),
                subtitle: const Text(FeatureFlags.splitScreenKey),
                onChanged: (value) {
                  controller.setFeatureFlag(FeatureFlags.splitScreenKey, value);
                },
              ),
              SwitchListTile(
                value: preferences.flags.enableAdBlocker,
                title: const Text('Ad blocker'),
                subtitle: const Text(FeatureFlags.adBlockerKey),
                onChanged: (value) {
                  controller.setFeatureFlag(FeatureFlags.adBlockerKey, value);
                },
              ),
              SwitchListTile(
                value: preferences.flags.enableAutoUpdate,
                title: const Text('Auto update'),
                subtitle: const Text(FeatureFlags.autoUpdateKey),
                onChanged: (value) {
                  controller.setFeatureFlag(FeatureFlags.autoUpdateKey, value);
                },
              ),
            ],
          );
        },
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String title;

  const _SectionHeader(this.title);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
      child: Text(
        title.toUpperCase(),
        style: Theme.of(context).textTheme.labelMedium?.copyWith(
              color: Theme.of(context).colorScheme.primary,
            ),
      ),
    );
  }
}
