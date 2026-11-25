import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/form_snapshot.dart';
import 'package:phantom_mobile/domain/preferences.dart';
import 'package:phantom_mobile/ui/browser/browser_controller.dart';
import 'package:phantom_mobile/ui/devtools/devtools_log_store.dart';
import 'package:phantom_mobile/webview/webview_host.dart';

import '../helpers/fakes.dart';

BrowserController buildController() {
  return BrowserController(
    tabStackRepository: InMemoryTabStackRepository(),
    preferencesRepository: InMemoryPreferencesRepository(),
    formSnapshotRepository: InMemoryFormSnapshotRepository(),
    devToolsLogStore: DevToolsLogStore(),
  );
}

Future<void> settle() async {
  await Future<void>.delayed(const Duration(milliseconds: 10));
}

void main() {
  group('BrowserController', () {
    test('restoreSession binds the space and restores a fresh stack', () async {
      final controller = buildController();
      await controller.restoreSession();
      expect(controller.tabs, hasLength(1));
      expect(controller.space.id, 'session');
    });

    test('navigating resolves the omnibox and records history', () async {
      final controller = buildController();
      await controller.restoreSession();
      await controller.navigate('flutter.dev');
      expect(controller.activeTab.url, 'https://flutter.dev');
      await controller.navigate('example.com');
      expect(controller.activeTab.url, 'https://example.com');
      expect(controller.activeTab.canGoBack, isTrue);
    });

    test('back and forward move through history', () async {
      final controller = buildController();
      await controller.restoreSession();
      await controller.navigate('https://a.example');
      await controller.navigate('https://b.example');
      await controller.back();
      expect(controller.activeTab.url, 'https://a.example');
      await controller.forward();
      expect(controller.activeTab.url, 'https://b.example');
    });

    test('addTab, selectTab and closeTab manage the stack', () async {
      final controller = buildController();
      await controller.restoreSession();
      final firstId = controller.activeTab.id;
      await controller.addTab();
      final secondId = controller.activeTab.id;
      expect(controller.tabs, hasLength(2));

      await controller.selectTab(firstId);
      expect(controller.activeTab.id, firstId);

      await controller.closeTab(firstId);
      expect(controller.tabs, hasLength(1));
      expect(controller.activeTab.id, secondId);

      await controller.closeTab(secondId);
      expect(controller.tabs, hasLength(1));
    });

    test('reorderTabs moves entries and keeps the active tab', () async {
      final controller = buildController();
      await controller.restoreSession();
      final firstId = controller.activeTab.id;
      await controller.addTab();
      final secondId = controller.activeTab.id;
      await controller.addTab();
      final thirdId = controller.activeTab.id;

      await controller.selectTab(firstId);
      await controller.reorderTabs(0, 2);

      expect(controller.tabs[0].id, secondId);
      expect(controller.tabs[1].id, firstId);
      expect(controller.tabs[2].id, thirdId);
      expect(controller.activeTab.id, firstId);

      await controller.reorderTabs(0, 3);
      expect(controller.tabs[0].id, firstId);
      expect(controller.tabs[1].id, thirdId);
      expect(controller.tabs[2].id, secondId);
    });

    test('the stack persists after every mutation', () async {
      final repository = InMemoryTabStackRepository();
      final controller = BrowserController(
        tabStackRepository: repository,
        preferencesRepository: InMemoryPreferencesRepository(),
        formSnapshotRepository: InMemoryFormSnapshotRepository(),
        devToolsLogStore: DevToolsLogStore(),
      );
      await controller.restoreSession();
      await controller.navigate('https://a.example');
      await controller.addTab();
      expect(repository.saveCount, 2);
      expect(repository.stored!.tabs, hasLength(2));
    });

    test('split view is gated by the feature flag', () async {
      final controller = buildController();
      await controller.restoreSession();
      await controller.toggleSplitView();
      expect(controller.splitViewActive, isFalse);

      await controller.setFeatureFlag(FeatureFlags.splitScreenKey, true);
      await controller.toggleSplitView();
      expect(controller.splitViewActive, isTrue);
      expect(controller.splitTabId, isNotNull);

      await controller.toggleSplitView();
      expect(controller.splitViewActive, isFalse);
    });

    test('feature flags persist through the repository', () async {
      final repository = InMemoryPreferencesRepository();
      final controller = BrowserController(
        tabStackRepository: InMemoryTabStackRepository(),
        preferencesRepository: repository,
        formSnapshotRepository: InMemoryFormSnapshotRepository(),
        devToolsLogStore: DevToolsLogStore(),
      );
      await controller.restoreSession();
      await controller.setFeatureFlag(FeatureFlags.adBlockerKey, true);
      await controller.setTheme(ThemePreference.dark);
      expect(repository.stored!.flags.enableAdBlocker, isTrue);
      expect(repository.stored!.theme, ThemePreference.dark);
      expect(controller.adBlockerEnabled, isTrue);
    });

    test('form drafts are captured before navigating away', () async {
      final formRepository = InMemoryFormSnapshotRepository();
      final controller = BrowserController(
        tabStackRepository: InMemoryTabStackRepository(),
        preferencesRepository: InMemoryPreferencesRepository(),
        formSnapshotRepository: formRepository,
        devToolsLogStore: DevToolsLogStore(),
      );
      final host = FakeWebViewHost();
      controller.attachHost(WebViewSlot.primary, host);
      await controller.restoreSession();
      await controller.navigate('https://first.example');
      await settle();

      host.formFields = <String, String>{'note': 'hello'};
      await controller.navigate('https://second.example');
      await settle();

      expect(host.loadedUrls, contains('https://second.example'));
      final draft = await formRepository.load('https://first.example');
      expect(draft, isNotNull);
      expect(draft!.fields['note'], 'hello');
    });

    test('page loads restore saved drafts through the host', () async {
      final formRepository = InMemoryFormSnapshotRepository();
      final controller = BrowserController(
        tabStackRepository: InMemoryTabStackRepository(),
        preferencesRepository: InMemoryPreferencesRepository(),
        formSnapshotRepository: formRepository,
        devToolsLogStore: DevToolsLogStore(),
      );
      final host = FakeWebViewHost();
      controller.attachHost(WebViewSlot.primary, host);
      await controller.restoreSession();
      await controller.navigate('https://draft.example');
      await settle();

      await formRepository.save(const FormSnapshot(
        pageUrl: 'https://draft.example',
        fields: <String, String>{'q': 'phantom'},
      ));
      controller.handleWebViewEvent(
        WebViewSlot.primary,
        const WebViewEvent(
          kind: WebViewEventKind.pageLoaded,
          url: 'https://draft.example',
        ),
      );
      await settle();

      expect(host.restored, isNotEmpty);
      expect(host.restored.first['q'], 'phantom');
    });

    test('url changes from the web view record history', () async {
      final controller = buildController();
      final host = FakeWebViewHost();
      controller.attachHost(WebViewSlot.primary, host);
      await controller.restoreSession();
      await controller.navigate('https://a.example');
      await settle();

      host.emit(const WebViewEvent(
        kind: WebViewEventKind.urlChanged,
        url: 'https://a.example/redirected',
      ));
      await settle();
      expect(controller.activeTab.url, 'https://a.example/redirected');
    });

    test('bridge messages feed the devtools log store', () async {
      final controller = buildController();
      final host = FakeWebViewHost();
      controller.attachHost(WebViewSlot.primary, host);
      await controller.restoreSession();

      host.emit(const WebViewEvent(
        kind: WebViewEventKind.bridgeMessage,
        message:
            '{"kind":"console","level":"log","message":"hello from the page"}',
      ));
      await settle();
      expect(controller.devToolsLogStore.console, hasLength(1));
      expect(
        controller.devToolsLogStore.console.first.message,
        'hello from the page',
      );
    });
  });
}
