import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:phantom_mobile/domain/space.dart';
import 'package:phantom_mobile/domain/tab_entry.dart';
import 'package:phantom_mobile/domain/tab_stack.dart';
import 'package:phantom_mobile/infra/persistence/directory_provider.dart';
import 'package:phantom_mobile/infra/persistence/hive_store.dart';
import 'package:phantom_mobile/infra/persistence/hive_tab_stack_repository.dart';

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
    tempRoot = await Directory('/tmp').createTemp('phantom_tabs_test');
    store = HiveStore(FakeDirectoryProvider(tempRoot));
  });

  tearDown(() async {
    await Hive.close();
    if (tempRoot.existsSync()) {
      tempRoot.deleteSync(recursive: true);
    }
  });

  group('HiveTabStackRepository', () {
    test('save then load restores the stack', () async {
      final repository = HiveTabStackRepository(store);
      await repository.bindSpace('session');
      var entry = TabEntry.blank(id: 't1');
      entry = entry.navigateTo('https://a.example').withTitle('A');
      final stack = TabStack.initial('session')
          .withTabs(<TabEntry>[entry], nextActiveId: 't1');
      await repository.save(stack);

      final restored = await repository.load();
      expect(restored, isNotNull);
      expect(restored!.activeTabId, 't1');
      expect(restored.tabs.single.url, 'https://a.example');
      expect(restored.tabs.single.title, 'A');
    });

    test('load on a fresh space returns null', () async {
      final repository = HiveTabStackRepository(store);
      await repository.bindSpace('fresh');
      expect(await repository.load(), isNull);
    });

    test('different spaces use different directories', () async {
      final repository = HiveTabStackRepository(store);
      final alphaDir = await store.useSpace(
        const Space(id: 'alpha', label: 'Alpha'),
      );
      await repository.bindSpace('alpha');
      expect(alphaDir.path, contains('spaces/alpha'));
      final betaDir = await store.useSpace(
        const Space(id: 'beta', label: 'Beta'),
      );
      expect(betaDir.path, contains('spaces/beta'));
      expect(betaDir.path, isNot(alphaDir.path));
    });

    test('spaces never read each other stacks', () async {
      final repository = HiveTabStackRepository(store);
      await repository.bindSpace('alpha');
      var entry = TabEntry.blank(id: 't1');
      entry = entry.navigateTo('https://alpha.example');
      await repository.save(
        TabStack.initial('alpha').withTabs(<TabEntry>[entry]),
      );

      await repository.bindSpace('beta');
      expect(await repository.load(), isNull);
      await repository.save(TabStack.initial('beta'));

      await repository.bindSpace('alpha');
      final restored = await repository.load();
      expect(restored!.tabs.single.url, 'https://alpha.example');
    });

    test('clear wipes the stored stack', () async {
      final repository = HiveTabStackRepository(store);
      await repository.bindSpace('session');
      await repository.save(TabStack.initial('session'));
      await repository.clear();
      expect(await repository.load(), isNull);
    });
  });
}
