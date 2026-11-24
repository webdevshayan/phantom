import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:phantom_mobile/domain/form_snapshot.dart';
import 'package:phantom_mobile/infra/persistence/directory_provider.dart';
import 'package:phantom_mobile/infra/persistence/hive_form_snapshot_repository.dart';
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
    tempRoot = await Directory('/tmp').createTemp('phantom_forms_test');
    store = HiveStore(FakeDirectoryProvider(tempRoot));
  });

  tearDown(() async {
    await Hive.close();
    if (tempRoot.existsSync()) {
      tempRoot.deleteSync(recursive: true);
    }
  });

  group('HiveFormSnapshotRepository', () {
    test('save then load restores the snapshot', () async {
      final repository = HiveFormSnapshotRepository(store);
      await repository.bindSpace('session');
      const snapshot = FormSnapshot(
        pageUrl: 'https://example.com/new-post',
        fields: <String, String>{'title': 'hello'},
      );
      await repository.save(snapshot);

      final restored = await repository.load('https://example.com/new-post');
      expect(restored, isNotNull);
      expect(restored!.fields['title'], 'hello');
    });

    test('spaces never read each other drafts', () async {
      final repository = HiveFormSnapshotRepository(store);
      await repository.bindSpace('alpha');
      await repository.save(const FormSnapshot(
        pageUrl: 'https://example.com/form',
        fields: <String, String>{'a': '1'},
      ));

      await repository.bindSpace('beta');
      expect(await repository.load('https://example.com/form'), isNull);
      await repository.save(const FormSnapshot(
        pageUrl: 'https://example.com/form',
        fields: <String, String>{'b': '2'},
      ));

      await repository.bindSpace('alpha');
      final restored = await repository.load('https://example.com/form');
      expect(restored!.fields['a'], '1');
      expect(restored.fields.containsKey('b'), isFalse);
    });

    test('clear removes the draft', () async {
      final repository = HiveFormSnapshotRepository(store);
      await repository.bindSpace('session');
      await repository.save(const FormSnapshot(
        pageUrl: 'https://example.com/form',
        fields: <String, String>{'a': '1'},
      ));
      await repository.clear('https://example.com/form');
      expect(await repository.load('https://example.com/form'), isNull);
    });
  });
}
