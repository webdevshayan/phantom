import 'dart:convert';

import 'package:hive/hive.dart';

import '../../domain/preferences.dart';
import '../../domain/repositories.dart';
import '../../domain/space.dart';
import 'hive_store.dart';

/// Stores preferences inside the Space's own data directory.
class HivePreferencesRepository implements PreferencesRepository {
  static const String boxName = 'preferences';
  static const String entryKey = 'app';

  final HiveStore store;
  Box<String>? _box;

  HivePreferencesRepository(this.store);

  @override
  Future<void> bindSpace(String spaceId) async {
    await store.useSpace(Space(id: spaceId, label: spaceId));
    _box = await store.openBox<String>(boxName);
  }

  @override
  Future<Preferences?> load() async {
    final raw = _box?.get(entryKey);
    if (raw == null) {
      return null;
    }
    try {
      return Preferences.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  @override
  Future<void> save(Preferences preferences) async {
    await _box?.put(entryKey, jsonEncode(preferences.toJson()));
  }
}
