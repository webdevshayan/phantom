import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/tab_entry.dart';
import 'package:phantom_mobile/domain/tab_stack.dart';

void main() {
  group('TabEntry', () {
    test('a blank tab has no back or forward entries', () {
      final tab = TabEntry.blank(id: 't1');
      expect(tab.url, TabEntry.blankUrl);
      expect(tab.canGoBack, isFalse);
      expect(tab.canGoForward, isFalse);
      expect(tab.title, 'New tab');
    });

    test('navigating appends history', () {
      var tab = TabEntry.blank(id: 't1');
      tab = tab.navigateTo('https://a.example');
      tab = tab.navigateTo('https://b.example');
      expect(
        tab.history,
        <String>[TabEntry.blankUrl, 'https://a.example', 'https://b.example'],
      );
      expect(tab.canGoBack, isTrue);
    });

    test('back and forward respect the stack bounds', () {
      var tab = TabEntry.blank(id: 't1');
      expect(tab.goBack().url, TabEntry.blankUrl);
      tab = tab.navigateTo('https://a.example');
      expect(tab.goBack().url, TabEntry.blankUrl);
      expect(tab.goForward().url, 'https://a.example');
      expect(tab.goForward().url, 'https://a.example');
    });

    test('navigating after going back trims the forward stack', () {
      var tab = TabEntry.blank(id: 't1');
      tab = tab.navigateTo('https://a.example');
      tab = tab.navigateTo('https://b.example');
      tab = tab.goBack();
      tab = tab.navigateTo('https://c.example');
      expect(
        tab.history,
        <String>[TabEntry.blankUrl, 'https://a.example', 'https://c.example'],
      );
      expect(tab.canGoForward, isFalse);
    });

    test('navigating to the current url is a no-op', () {
      var tab = TabEntry.blank(id: 't1');
      tab = tab.navigateTo('https://a.example');
      expect(identical(tab.navigateTo('https://a.example'), tab), isTrue);
    });

    test('withTitle keeps the history intact', () {
      var tab = TabEntry.blank(id: 't1');
      tab = tab.navigateTo('https://a.example');
      final titled = tab.withTitle('Example');
      expect(titled.title, 'Example');
      expect(titled.history, tab.history);
      expect(titled.historyIndex, tab.historyIndex);
    });

    test('json round trip preserves the entry', () {
      var tab = TabEntry.blank(id: 't1');
      tab = tab.navigateTo('https://a.example').withTitle('Example');
      final restored = TabEntry.fromJson(tab.toJson());
      expect(restored.id, 't1');
      expect(restored.title, 'Example');
      expect(restored.history, tab.history);
      expect(restored.historyIndex, tab.historyIndex);
    });
  });

  group('TabStack', () {
    test('initial stack has one active blank tab', () {
      final stack = TabStack.initial('session');
      expect(stack.tabs, hasLength(1));
      expect(stack.active.url, TabEntry.blankUrl);
      expect(stack.spaceId, 'session');
    });

    test('byId finds tabs or returns null', () {
      final stack = TabStack.initial('session');
      expect(stack.byId(stack.activeTabId), isNotNull);
      expect(stack.byId('missing'), isNull);
    });

    test('withTabs replaces the list and the active id', () {
      final first = TabStack.initial('session');
      final second = TabEntry.blank(id: 't2');
      final updated = first.withTabs(
        <TabEntry>[...first.tabs, second],
        nextActiveId: 't2',
      );
      expect(updated.tabs, hasLength(2));
      expect(updated.active.id, 't2');
      expect(updated.spaceId, 'session');
    });

    test('json round trip preserves the stack', () {
      var entry = TabEntry.blank(id: 't1');
      entry = entry.navigateTo('https://a.example');
      final stack = TabStack.initial('session').withTabs(
        <TabEntry>[entry],
        nextActiveId: 't1',
      );
      final restored = TabStack.fromJson(stack.toJson());
      expect(restored.spaceId, 'session');
      expect(restored.activeTabId, 't1');
      expect(restored.tabs.single.url, 'https://a.example');
    });
  });
}
