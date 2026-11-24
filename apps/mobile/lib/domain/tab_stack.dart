import 'package:flutter/foundation.dart';

import 'tab_entry.dart';

/// The full set of open tabs for one Space. Persisted through Hive so the
/// shell can restore the whole stack after a restart.
@immutable
class TabStack {
  final String spaceId;
  final List<TabEntry> tabs;
  final String activeTabId;

  const TabStack({
    required this.spaceId,
    required this.tabs,
    required this.activeTabId,
  });

  factory TabStack.initial(String spaceId) {
    final entry = TabEntry.blank();
    return TabStack(
      spaceId: spaceId,
      tabs: <TabEntry>[entry],
      activeTabId: entry.id,
    );
  }

  TabEntry get active {
    for (final tab in tabs) {
      if (tab.id == activeTabId) {
        return tab;
      }
    }
    return tabs.first;
  }

  TabEntry? byId(String id) {
    for (final tab in tabs) {
      if (tab.id == id) {
        return tab;
      }
    }
    return null;
  }

  TabStack withTabs(List<TabEntry> nextTabs, {String? nextActiveId}) =>
      TabStack(
        spaceId: spaceId,
        tabs: List<TabEntry>.unmodifiable(nextTabs),
        activeTabId: nextActiveId ?? activeTabId,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'spaceId': spaceId,
        'activeTabId': activeTabId,
        'tabs': tabs.map((tab) => tab.toJson()).toList(),
      };

  factory TabStack.fromJson(Map<String, dynamic> json) {
    final spaceId = json['spaceId'];
    final activeId = json['activeTabId'];
    final rawTabs = json['tabs'];
    return TabStack(
      spaceId: spaceId is String ? spaceId : 'session',
      activeTabId: activeId is String ? activeId : '',
      tabs: rawTabs is List
          ? rawTabs
              .whereType<Map>()
              .map((item) => TabEntry.fromJson(Map<String, dynamic>.from(item)))
              .toList()
          : <TabEntry>[],
    );
  }
}
