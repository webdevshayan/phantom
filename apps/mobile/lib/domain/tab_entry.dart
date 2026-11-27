import 'package:flutter/foundation.dart';

/// One open tab: its title plus the navigation history that lets the tab
/// stack be restored after a restart.
@immutable
class TabEntry {
  final String id;
  final String title;
  final List<String> history;
  final int historyIndex;

  const TabEntry({
    required this.id,
    required this.title,
    this.history = const <String>[],
    this.historyIndex = -1,
  });

  /// Placeholder location of a tab that has not navigated yet.
  static const String blankUrl = 'about:blank';

  factory TabEntry.blank({String? id, String url = blankUrl}) {
    final resolvedId = id ?? 'tab-${DateTime.now().microsecondsSinceEpoch}';
    return TabEntry(
      id: resolvedId,
      title: 'New tab',
      history: <String>[url],
      historyIndex: 0,
    );
  }

  String get url => history.isEmpty
      ? blankUrl
      : history[historyIndex.clamp(0, history.length - 1)];

  bool get canGoBack => historyIndex > 0;

  bool get canGoForward =>
      historyIndex >= 0 && historyIndex < history.length - 1;

  /// Moves to [nextUrl], trimming any forward entries so the back stack
  /// never forks.
  TabEntry navigateTo(String nextUrl) {
    if (nextUrl == url) {
      return this;
    }
    final trimmed = List<String>.from(history.take(historyIndex + 1));
    trimmed.add(nextUrl);
    return TabEntry(
      id: id,
      title: title,
      history: trimmed,
      historyIndex: trimmed.length - 1,
    );
  }

  /// Re-targets the current position to [nextUrl] without touching the
  /// surrounding history. The engine folds a server-side redirect into the
  /// entry the navigation created instead of appending one, so a redirect
  /// hop replaces the provisional url at the current position: the page
  /// before it stays one step back, no phantom position is added.
  TabEntry withUrl(String nextUrl) {
    if (nextUrl == url || history.isEmpty) {
      return this;
    }
    final index = historyIndex.clamp(0, history.length - 1);
    final next = List<String>.from(history);
    next[index] = nextUrl;
    return TabEntry(
      id: id,
      title: title,
      history: next,
      historyIndex: index,
    );
  }

  TabEntry goBack() => canGoBack ? _at(historyIndex - 1) : this;

  TabEntry goForward() => canGoForward ? _at(historyIndex + 1) : this;

  TabEntry withTitle(String nextTitle) => TabEntry(
        id: id,
        title: nextTitle,
        history: history,
        historyIndex: historyIndex,
      );

  TabEntry _at(int index) => TabEntry(
        id: id,
        title: title,
        history: history,
        historyIndex: index,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'title': title,
        'history': history,
        'historyIndex': historyIndex,
      };

  factory TabEntry.fromJson(Map<String, dynamic> json) {
    final id = json['id'];
    final rawHistory = json['history'];
    final history = rawHistory is List
        ? rawHistory.whereType<String>().toList()
        : <String>[];
    return TabEntry(
      id: id is String ? id : 'tab-unknown',
      title: json['title'] is String ? json['title'] as String : 'New tab',
      history: history,
      historyIndex: (json['historyIndex'] as num?)?.toInt() ?? 0,
    );
  }
}
