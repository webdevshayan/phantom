import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../domain/form_snapshot.dart';
import '../../domain/omnibox.dart';
import '../../domain/preferences.dart';
import '../../domain/repositories.dart';
import '../../domain/search_engine.dart';
import '../../domain/space.dart';
import '../../domain/tab_entry.dart';
import '../../domain/tab_stack.dart';
import '../../webview/webview_host.dart';
import '../devtools/devtools_log_store.dart';

enum WebViewSlot { primary, secondary }

/// Owns the browser session for one Space: the tab stack, the preferences,
/// the attached web view hosts, and every persisted side effect.
class BrowserController extends ChangeNotifier {
  BrowserController({
    required this.tabStackRepository,
    required this.preferencesRepository,
    required this.formSnapshotRepository,
    required this.devToolsLogStore,
    this.space = Space.session,
  });

  final TabStackRepository tabStackRepository;
  final PreferencesRepository preferencesRepository;
  final FormSnapshotRepository formSnapshotRepository;
  final DevToolsLogStore devToolsLogStore;
  final Space space;

  TabStack? _stack;
  Preferences _preferences = Preferences.initial;
  WebViewHost? _primaryHost;
  StreamSubscription<WebViewEvent>? _primaryEvents;
  StreamSubscription<WebViewEvent>? _secondaryEvents;
  bool _splitActive = false;
  String? _splitTabId;
  bool _suppressUrlEvents = false;
  int _idSequence = 0;

  TabStack? get stack => _stack;
  Preferences get preferences => _preferences;
  SearchEngine get searchEngine => SearchEngine.byId(_preferences.searchEngine);
  bool get adBlockerEnabled => _preferences.flags.enableAdBlocker;
  bool get splitViewActive =>
      _splitActive && _preferences.flags.enableSplitScreen;
  String? get splitTabId => _splitTabId;

  List<TabEntry> get tabs => _stack?.tabs ?? const <TabEntry>[];

  TabEntry get activeTab {
    final stack = _stack;
    if (stack == null || stack.tabs.isEmpty) {
      return TabEntry.blank();
    }
    return stack.active;
  }

  String get splitTabUrl {
    final id = _splitTabId;
    final tab = id == null ? null : _stack?.byId(id);
    return tab?.url ?? searchEngine.homeUrl;
  }

  Future<void> restoreSession() async {
    await tabStackRepository.bindSpace(space.id);
    await preferencesRepository.bindSpace(space.id);
    await formSnapshotRepository.bindSpace(space.id);
    _preferences = (await preferencesRepository.load()) ?? Preferences.initial;
    _stack = (await tabStackRepository.load()) ?? TabStack.initial(space.id);
    notifyListeners();
    final url = activeTab.url;
    if (url != TabEntry.blankUrl) {
      await _loadOn(_primaryHost, url);
    }
  }

  Future<void> navigate(String rawInput) async {
    final target = resolveOmniboxInput(rawInput, searchEngine);
    await _captureFormDrafts();
    _replaceTab(activeTab.navigateTo(target.url));
    notifyListeners();
    await _loadOn(_primaryHost, target.url);
    await _persistStack();
  }

  Future<void> back() async {
    final current = activeTab;
    if (!current.canGoBack) {
      return;
    }
    final previous = current.goBack();
    _replaceTab(previous);
    _suppressUrlEvents = true;
    notifyListeners();
    await _loadOn(_primaryHost, previous.url);
    _suppressUrlEvents = false;
    await _persistStack();
  }

  Future<void> forward() async {
    final current = activeTab;
    if (!current.canGoForward) {
      return;
    }
    final next = current.goForward();
    _replaceTab(next);
    _suppressUrlEvents = true;
    notifyListeners();
    await _loadOn(_primaryHost, next.url);
    _suppressUrlEvents = false;
    await _persistStack();
  }

  Future<void> reload() async {
    final host = _primaryHost;
    if (host == null) {
      return;
    }
    try {
      await host.reload();
    } on Exception {
      // Reload failures surface through web resource error events.
    }
  }

  Future<void> addTab() async {
    final entry = TabEntry.blank(id: _nextTabId(), url: searchEngine.homeUrl);
    final stack = _stack ?? TabStack.initial(space.id);
    _stack = stack.withTabs(
      <TabEntry>[...stack.tabs, entry],
      nextActiveId: entry.id,
    );
    notifyListeners();
    await _loadOn(_primaryHost, entry.url);
    await _persistStack();
  }

  Future<void> selectTab(String tabId) async {
    final stack = _stack;
    if (stack == null || stack.byId(tabId) == null) {
      return;
    }
    _stack = stack.withTabs(stack.tabs, nextActiveId: tabId);
    notifyListeners();
    await _loadOn(_primaryHost, activeTab.url);
    await _persistStack();
  }

  Future<void> closeTab(String tabId) async {
    final stack = _stack;
    if (stack == null || stack.byId(tabId) == null) {
      return;
    }
    final wasActive = tabId == stack.activeTabId;
    final remaining = stack.tabs.where((tab) => tab.id != tabId).toList();
    if (remaining.isEmpty) {
      final fresh = TabEntry.blank(id: _nextTabId(), url: searchEngine.homeUrl);
      _stack = TabStack(
        spaceId: stack.spaceId,
        tabs: <TabEntry>[fresh],
        activeTabId: fresh.id,
      );
      if (tabId == _splitTabId) {
        _splitTabId = null;
        _splitActive = false;
      }
      notifyListeners();
      await _loadOn(_primaryHost, fresh.url);
      await _persistStack();
      return;
    }
    var nextActiveId = stack.activeTabId;
    if (wasActive) {
      final index = stack.tabs.indexWhere((tab) => tab.id == tabId);
      nextActiveId = remaining[index.clamp(0, remaining.length - 1)].id;
    }
    if (tabId == _splitTabId) {
      _splitTabId = null;
      _splitActive = false;
    }
    _stack = stack.withTabs(remaining, nextActiveId: nextActiveId);
    notifyListeners();
    if (wasActive) {
      await _loadOn(_primaryHost, activeTab.url);
    }
    await _persistStack();
  }

  Future<void> reorderTabs(int oldIndex, int newIndex) async {
    final stack = _stack;
    if (stack == null || oldIndex < 0 || oldIndex >= stack.tabs.length) {
      return;
    }
    final tabs = List<TabEntry>.from(stack.tabs);
    final adjusted = newIndex > oldIndex ? newIndex - 1 : newIndex;
    final moved = tabs.removeAt(oldIndex);
    tabs.insert(adjusted.clamp(0, tabs.length), moved);
    _stack = stack.withTabs(tabs);
    notifyListeners();
    await _persistStack();
  }

  Future<void> setTheme(ThemePreference theme) async {
    _preferences = _preferences.copyWith(theme: theme);
    notifyListeners();
    await preferencesRepository.save(_preferences);
  }

  Future<void> setSearchEngine(SearchEngineId engine) async {
    _preferences = _preferences.copyWith(searchEngine: engine);
    notifyListeners();
    await preferencesRepository.save(_preferences);
  }

  Future<void> setFeatureFlag(String key, bool value) async {
    _preferences = _preferences.copyWith(
      flags: _preferences.flags.withValue(key, value),
    );
    notifyListeners();
    await preferencesRepository.save(_preferences);
  }

  Future<void> toggleSplitView() async {
    if (!_preferences.flags.enableSplitScreen) {
      return;
    }
    if (_splitActive) {
      _splitActive = false;
      _splitTabId = null;
      notifyListeners();
      return;
    }
    final stack = _stack;
    if (stack == null) {
      return;
    }
    final otherId = _otherTabId(stack);
    var other = otherId == null ? null : stack.byId(otherId);
    if (other == null) {
      other = TabEntry.blank(id: _nextTabId(), url: searchEngine.homeUrl);
      _stack = stack.withTabs(<TabEntry>[...stack.tabs, other]);
    }
    _splitTabId = other.id;
    _splitActive = true;
    notifyListeners();
    await _persistStack();
  }

  String? _otherTabId(TabStack stack) {
    for (final tab in stack.tabs) {
      if (tab.id != stack.activeTabId) {
        return tab.id;
      }
    }
    return null;
  }

  void attachHost(WebViewSlot slot, WebViewHost host) {
    if (slot == WebViewSlot.primary) {
      _primaryEvents?.cancel();
      _primaryHost = host;
      _primaryEvents = host.events
          .listen((event) => handleWebViewEvent(WebViewSlot.primary, event));
    } else {
      _secondaryEvents?.cancel();
      _secondaryEvents = host.events
          .listen((event) => handleWebViewEvent(WebViewSlot.secondary, event));
    }
  }

  void detachHost(WebViewSlot slot) {
    if (slot == WebViewSlot.primary) {
      _primaryEvents?.cancel();
      _primaryEvents = null;
      _primaryHost = null;
    } else {
      _secondaryEvents?.cancel();
      _secondaryEvents = null;
    }
  }

  void handleWebViewEvent(WebViewSlot slot, WebViewEvent event) {
    if (event.kind == WebViewEventKind.bridgeMessage) {
      final message = event.message;
      if (message != null) {
        devToolsLogStore.handleRaw(message);
      }
      return;
    }
    if (event.kind == WebViewEventKind.loadFailed) {
      if (slot == WebViewSlot.primary && event.message != null) {
        devToolsLogStore.appendConsole(
          'error',
          'Failed to load: ${event.message}',
        );
      }
      return;
    }
    if (slot != WebViewSlot.primary || _suppressUrlEvents) {
      return;
    }
    final url = event.url;
    if (url != null && url != activeTab.url) {
      _replaceTab(activeTab.navigateTo(url));
      notifyListeners();
      unawaited(_persistStack());
    }
    if (event.kind == WebViewEventKind.pageLoaded) {
      unawaited(restoreFormDrafts());
    }
  }

  Future<void> captureFormDrafts() => _captureFormDrafts();

  Future<void> restoreFormDrafts() async {
    final host = _primaryHost;
    if (host == null) {
      return;
    }
    final pageUrl = activeTab.url;
    if (pageUrl == TabEntry.blankUrl) {
      return;
    }
    try {
      final snapshot = await formSnapshotRepository.load(pageUrl);
      if (snapshot != null && snapshot.fields.isNotEmpty) {
        await host.restoreFormFields(snapshot.fields);
      }
    } on Exception {
      // Restore is best effort.
    }
  }

  Future<void> _captureFormDrafts() async {
    final host = _primaryHost;
    if (host == null) {
      return;
    }
    final pageUrl = activeTab.url;
    if (pageUrl == TabEntry.blankUrl) {
      return;
    }
    try {
      final fields = await host.captureFormFields();
      if (fields.isEmpty) {
        await formSnapshotRepository.clear(pageUrl);
      } else {
        await formSnapshotRepository.save(
          FormSnapshot(pageUrl: pageUrl, fields: fields),
        );
      }
    } on Exception {
      // Draft capture is best effort.
    }
  }

  Future<void> _loadOn(WebViewHost? host, String url) async {
    if (host == null) {
      return;
    }
    try {
      await host.load(url);
    } on Exception {
      // Navigation failures surface through web view events.
    }
  }

  void _replaceTab(TabEntry tab) {
    final stack = _stack;
    if (stack == null) {
      return;
    }
    _stack = stack.withTabs(<TabEntry>[
      for (final entry in stack.tabs) entry.id == tab.id ? tab : entry,
    ]);
  }

  String _nextTabId() {
    _idSequence += 1;
    return 'tab-${space.id}-$_idSequence';
  }

  Future<void> _persistStack() async {
    final stack = _stack;
    if (stack == null) {
      return;
    }
    await tabStackRepository.save(stack);
  }

  @override
  void dispose() {
    _primaryEvents?.cancel();
    _secondaryEvents?.cancel();
    super.dispose();
  }
}
