import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:phantom_mobile/domain/preferences.dart';
import 'package:phantom_mobile/infra/persistence/directory_provider.dart';
import 'package:phantom_mobile/infra/persistence/hive_preferences_repository.dart';
import 'package:phantom_mobile/infra/persistence/hive_store.dart';

class FakeDirectoryProvider implements DirectoryProvider {
  final Directory root;

  FakeDirectoryProvider(this.root);

  @override
  Future<Directory> dataRoot() async => root;
}

void main() {
  late Directory tempRoot;
  late HiveStore store;

  setUp(() async {
    tempRoot = await Directory('/tmp').createTemp('phantom_prefs_test');
    store = HiveStore(FakeDirectoryProvider(tempRoot));
  });

  tearDown(() async {
    await Hive.close();
    if (tempRoot.existsSync()) {
      tempRoot.deleteSync(recursive: true);
    }
  });

  group('HivePreferencesRepository', () {
    test('load on a fresh space returns null', () async {
      final repository = HivePreferencesRepository(store);
      await repository.bindSpace('fresh');
      expect(await repository.load(), isNull);
    });

    test('save then load restores preferences', () async {
      final repository = HivePreferencesRepository(store);
      await repository.bindSpace('session');
      final preferences = Preferences.initial.copyWith(
        theme: ThemePreference.dark,
        searchEngine: SearchEngineId.duckDuckGo,
        flags: Preferences.initial.flags
            .withValue(FeatureFlags.splitScreenKey, true),
      );
      await repository.save(preferences);

      final restored = await repository.load();
      expect(restored, isNotNull);
      expect(restored!.theme, ThemePreference.dark);
      expect(restored.searchEngine, SearchEngineId.duckDuckGo);
      expect(restored.flags.enableSplitScreen, isTrue);
    });

    test('spaces never read each other preferences', () async {
      final repository = HivePreferencesRepository(store);
      await repository.bindSpace('alpha');
      await repository.save(
        Preferences.initial.copyWith(theme: ThemePreference.dark),
      );

      await repository.bindSpace('beta');
      expect(await repository.load(), isNull);
      await repository.save(Preferences.initial);

      await repository.bindSpace('alpha');
      final restored = await repository.load();
      expect(restored!.theme, ThemePreference.dark);
    });
  });
}
