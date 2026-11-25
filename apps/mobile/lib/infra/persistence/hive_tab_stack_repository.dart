import 'dart:convert';

import 'package:hive/hive.dart';

import '../../domain/repositories.dart';
import '../../domain/space.dart';
import '../../domain/tab_stack.dart';
import 'hive_store.dart';

/// Stores the tab stack inside the Space's own data directory.
class HiveTabStackRepository implements TabStackRepository {
  static const String boxName = 'tab_stacks';
  static const String entryKey = 'active_stack';

  final HiveStore store;
  Box<String>? _box;

  HiveTabStackRepository(this.store);

  @override
  Future<void> bindSpace(String spaceId) async {
    await store.useSpace(Space(id: spaceId, label: spaceId));
    _box = await store.openBox<String>(boxName);
  }

  @override
  Future<TabStack?> load() async {
    final raw = _box?.get(entryKey);
    if (raw == null) {
      return null;
    }
    try {
      return TabStack.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    } catch (_) {
      // Corrupt entries fall back to a fresh stack instead of crashing.
      return null;
    }
  }

  @override
  Future<void> save(TabStack stack) async {
    await _box?.put(entryKey, jsonEncode(stack.toJson()));
  }

  @override
  Future<void> clear() async {
    await _box?.delete(entryKey);
  }
}
