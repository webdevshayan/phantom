import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/webview/devtools_bridge.dart';
import 'package:phantom_mobile/webview/form_capture_bridge.dart';
import 'package:phantom_mobile/webview/form_restore_bridge.dart';

void main() {
  group('DevToolsBridge', () {
    test('script targets the dedicated channel', () {
      final script = DevToolsBridge.build();
      expect(script, contains(DevToolsBridge.channelName));
      expect(script, contains('__phantomDevToolsInstalled'));
    });

    test('script hooks console, errors and network primitives', () {
      final script = DevToolsBridge.build();
      expect(script, contains("['log', 'info', 'warn', 'error']"));
      expect(script, contains('addEventListener(\'error\''));
      expect(script, contains('window.fetch'));
      expect(script, contains('XMLHttpRequest'));
      expect(script, contains('JSON.stringify'));
    });
  });

  group('FormCaptureBridge', () {
    test('script collects safe fields only', () {
      final script = FormCaptureBridge.build();
      expect(script, contains('querySelectorAll(\'input, textarea\')'));
      expect(script, contains('password'));
      expect(script, contains('hidden'));
      expect(script, contains('JSON.stringify'));
    });
  });

  group('FormRestoreBridge', () {
    test('script embeds the payload and guards secret fields', () {
      final script = FormRestoreBridge.build(<String, String>{'q': 'phantom'});
      expect(script, contains('"q":"phantom"'));
      expect(script, contains('password'));
      expect(script, contains('hasOwnProperty'));
    });

    test('quoted values survive json encoding', () {
      final script = FormRestoreBridge.build(<String, String>{
        'note': 'a "quoted" value',
      });
      expect(script, contains(r'"note":"a \"quoted\" value"'));
    });
  });
}
