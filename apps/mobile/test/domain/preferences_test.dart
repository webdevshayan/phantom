import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/preferences.dart';

void main() {
  group('FeatureFlags', () {
    test('every flag defaults to off', () {
      const flags = FeatureFlags.allOff;
      expect(flags.enableSplitScreen, isFalse);
      expect(flags.enableAdBlocker, isFalse);
      expect(flags.enableAutoUpdate, isFalse);
    });

    test('toMap uses the exact persisted keys', () {
      const flags = FeatureFlags.allOff;
      expect(
        flags.toMap().keys,
        const <String>{
          'enable_split_screen',
          'enable_ad_blocker',
          'enable_auto_update',
        },
      );
    });

    test('withValue flips exactly one flag', () {
      const flags = FeatureFlags.allOff;
      final enabled = flags.withValue(FeatureFlags.splitScreenKey, true);
      expect(enabled.enableSplitScreen, isTrue);
      expect(enabled.enableAdBlocker, isFalse);
      expect(enabled.enableAutoUpdate, isFalse);
      expect(flags.valueOf(FeatureFlags.splitScreenKey), isFalse);
    });

    test('fromMap ignores unknown keys', () {
      final flags = FeatureFlags.fromMap(const <String, dynamic>{
        'enable_ad_blocker': true,
        'enable_totally_unknown': true,
      });
      expect(flags.enableAdBlocker, isTrue);
      expect(flags.enableSplitScreen, isFalse);
    });
  });

  group('Preferences', () {
    test('defaults keep every experiment off', () {
      const preferences = Preferences.initial;
      expect(preferences.theme, ThemePreference.followDevice);
      expect(preferences.searchEngine, SearchEngineId.google);
      expect(preferences.flags.enableSplitScreen, isFalse);
    });

    test('json round trip preserves all fields', () {
      final preferences = Preferences.initial.copyWith(
        theme: ThemePreference.dark,
        searchEngine: SearchEngineId.duckDuckGo,
        flags: FeatureFlags.allOff.withValue(FeatureFlags.splitScreenKey, true),
      );
      final restored = Preferences.fromJson(preferences.toJson());
      expect(restored.theme, ThemePreference.dark);
      expect(restored.searchEngine, SearchEngineId.duckDuckGo);
      expect(restored.flags.enableSplitScreen, isTrue);
    });

    test('corrupt json falls back to defaults', () {
      final restored = Preferences.fromJson(const <String, dynamic>{
        'theme': 'neon',
        'searchEngine': 42,
        'flags': 'nope',
      });
      expect(restored.theme, ThemePreference.followDevice);
      expect(restored.searchEngine, SearchEngineId.google);
      expect(restored.flags.enableAdBlocker, isFalse);
      expect(
        Preferences.initial.flags.toMap().keys,
        const <String>{
          'enable_split_screen',
          'enable_ad_blocker',
          'enable_auto_update',
        },
      );
    });

    test('labels cover every option', () {
      for (final preference in ThemePreference.values) {
        expect(preference.label, isNotEmpty);
      }
    });
  });
}
