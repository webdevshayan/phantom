import 'package:flutter/foundation.dart';

enum ThemePreference { light, dark, followDevice }

extension ThemePreferenceLabel on ThemePreference {
  String get label => switch (this) {
        ThemePreference.light => 'Light',
        ThemePreference.dark => 'Dark',
        ThemePreference.followDevice => 'Follow device',
      };
}

enum SearchEngineId { google, duckDuckGo, bing, startpage }

/// The three experimental capabilities exposed on the settings page. Keys
/// match the persisted map exactly so the settings file stays stable.
@immutable
class FeatureFlags {
  final bool enableSplitScreen;
  final bool enableAdBlocker;
  final bool enableAutoUpdate;

  const FeatureFlags({
    this.enableSplitScreen = false,
    this.enableAdBlocker = false,
    this.enableAutoUpdate = false,
  });

  static const FeatureFlags allOff = FeatureFlags();

  static const String splitScreenKey = 'enable_split_screen';
  static const String adBlockerKey = 'enable_ad_blocker';
  static const String autoUpdateKey = 'enable_auto_update';

  bool valueOf(String key) => switch (key) {
        splitScreenKey => enableSplitScreen,
        adBlockerKey => enableAdBlocker,
        autoUpdateKey => enableAutoUpdate,
        _ => false,
      };

  FeatureFlags withValue(String key, bool value) => switch (key) {
        splitScreenKey => copyWith(enableSplitScreen: value),
        adBlockerKey => copyWith(enableAdBlocker: value),
        autoUpdateKey => copyWith(enableAutoUpdate: value),
        _ => this,
      };

  Map<String, bool> toMap() => <String, bool>{
        splitScreenKey: enableSplitScreen,
        adBlockerKey: enableAdBlocker,
        autoUpdateKey: enableAutoUpdate,
      };

  factory FeatureFlags.fromMap(Map<String, dynamic> map) => FeatureFlags(
        enableSplitScreen: map[splitScreenKey] as bool? ?? false,
        enableAdBlocker: map[adBlockerKey] as bool? ?? false,
        enableAutoUpdate: map[autoUpdateKey] as bool? ?? false,
      );

  FeatureFlags copyWith({
    bool? enableSplitScreen,
    bool? enableAdBlocker,
    bool? enableAutoUpdate,
  }) =>
      FeatureFlags(
        enableSplitScreen: enableSplitScreen ?? this.enableSplitScreen,
        enableAdBlocker: enableAdBlocker ?? this.enableAdBlocker,
        enableAutoUpdate: enableAutoUpdate ?? this.enableAutoUpdate,
      );
}

/// Everything the settings page persists. Defaults keep every experiment
/// off until the user opts in.
@immutable
class Preferences {
  final ThemePreference theme;
  final SearchEngineId searchEngine;
  final FeatureFlags flags;

  const Preferences({
    this.theme = ThemePreference.followDevice,
    this.searchEngine = SearchEngineId.google,
    this.flags = FeatureFlags.allOff,
  });

  static const Preferences initial = Preferences();

  Preferences copyWith({
    ThemePreference? theme,
    SearchEngineId? searchEngine,
    FeatureFlags? flags,
  }) =>
      Preferences(
        theme: theme ?? this.theme,
        searchEngine: searchEngine ?? this.searchEngine,
        flags: flags ?? this.flags,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'theme': theme.name,
        'searchEngine': searchEngine.name,
        'flags': flags.toMap(),
      };

  factory Preferences.fromJson(Map<String, dynamic> json) {
    final themeName = json['theme'];
    final engineName = json['searchEngine'];
    final rawFlags = json['flags'];
    return Preferences(
      theme: themeName is String &&
              ThemePreference.values.any((value) => value.name == themeName)
          ? ThemePreference.values.byName(themeName)
          : ThemePreference.followDevice,
      searchEngine: engineName is String &&
              SearchEngineId.values.any((value) => value.name == engineName)
          ? SearchEngineId.values.byName(engineName)
          : SearchEngineId.google,
      flags: rawFlags is Map
          ? FeatureFlags.fromMap(Map<String, dynamic>.from(rawFlags))
          : FeatureFlags.allOff,
    );
  }
}
