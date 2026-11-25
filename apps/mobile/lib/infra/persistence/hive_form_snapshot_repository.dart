import 'dart:convert';

import 'package:hive/hive.dart';

import '../../domain/form_snapshot.dart';
import '../../domain/repositories.dart';
import '../../domain/space.dart';
import 'hive_store.dart';

/// Stores captured form drafts keyed by page URL, always inside the
/// Space's own data directory.
class HiveFormSnapshotRepository implements FormSnapshotRepository {
  static const String boxName = 'form_snapshots';

  final HiveStore store;
  Box<String>? _box;

  HiveFormSnapshotRepository(this.store);

  @override
  Future<void> bindSpace(String spaceId) async {
    await store.useSpace(Space(id: spaceId, label: spaceId));
    _box = await store.openBox<String>(boxName);
  }

  @override
  Future<FormSnapshot?> load(String pageUrl) async {
    final raw = _box?.get(pageUrl);
    if (raw == null) {
      return null;
    }
    try {
      return FormSnapshot.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  @override
  Future<void> save(FormSnapshot snapshot) async {
    await _box?.put(snapshot.pageUrl, jsonEncode(snapshot.toJson()));
  }

  @override
  Future<void> clear(String pageUrl) async {
    await _box?.delete(pageUrl);
  }
}
