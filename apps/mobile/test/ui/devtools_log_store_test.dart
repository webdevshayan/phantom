import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/ui/devtools/devtools_log_store.dart';

void main() {
  group('DevToolsLogStore', () {
    test('parses console payloads', () {
      final store = DevToolsLogStore();
      store.handleRaw(
        '{"kind":"console","level":"warn","message":"careful","at":1700000000000}',
      );
      expect(store.console, hasLength(1));
      expect(store.console.first.level, 'warn');
      expect(store.console.first.message, 'careful');
      expect(store.network, isEmpty);
    });

    test('parses network payloads', () {
      final store = DevToolsLogStore();
      store.handleRaw(
        '{"kind":"network","method":"POST","url":"https://example.com/api","status":200,"durationMs":42,"at":1700000000000}',
      );
      expect(store.network, hasLength(1));
      expect(store.network.first.method, 'POST');
      expect(store.network.first.status, 200);
      expect(store.network.first.durationMs, 42);
    });

    test('ignores junk payloads', () {
      final store = DevToolsLogStore();
      store.handleRaw('not json');
      store.handleRaw('["array"]');
      store.handleRaw('{"kind":"unknown"}');
      expect(store.console, isEmpty);
      expect(store.network, isEmpty);
    });

    test('appendConsole records load failures', () {
      final store = DevToolsLogStore();
      store.appendConsole('error', 'Failed to load: net::ERR');
      expect(store.console.single.level, 'error');
    });

    test('capacity caps both lists', () {
      final store = DevToolsLogStore(capacity: 5);
      for (var i = 0; i < 8; i++) {
        store.handleRaw('{"kind":"console","level":"log","message":"m$i"}');
        store.handleRaw('{"kind":"network","method":"GET","url":"u$i"}');
      }
      expect(store.console, hasLength(5));
      expect(store.console.first.message, 'm3');
      expect(store.console.last.message, 'm7');
      expect(store.network, hasLength(5));
    });

    test('clear empties the store and notifies listeners', () {
      final store = DevToolsLogStore();
      var notifications = 0;
      store.addListener(() {
        notifications++;
      });
      store.appendConsole('log', 'a');
      store.clear();
      expect(store.console, isEmpty);
      expect(notifications, 2);
    });
  });
}
