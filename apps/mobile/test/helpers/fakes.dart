import 'dart:async';

import 'package:phantom_mobile/domain/form_snapshot.dart';
import 'package:phantom_mobile/domain/preferences.dart';
import 'package:phantom_mobile/domain/repositories.dart';
import 'package:phantom_mobile/domain/tab_stack.dart';
import 'package:phantom_mobile/webview/webview_host.dart';

/// In-memory persistence used by controller tests.
class InMemoryTabStackRepository implements TabStackRepository {
  TabStack? stored;
  int saveCount = 0;
  String? boundSpaceId;

  @override
  Future<void> bindSpace(String spaceId) async {
    boundSpaceId = spaceId;
  }

  @override
  Future<TabStack?> load() async => stored;

  @override
  Future<void> save(TabStack stack) async {
    saveCount++;
    stored = stack;
  }

  @override
  Future<void> clear() async {
    stored = null;
  }
}

class InMemoryPreferencesRepository implements PreferencesRepository {
  Preferences? stored;
  int saveCount = 0;
  String? boundSpaceId;

  @override
  Future<void> bindSpace(String spaceId) async {
    boundSpaceId = spaceId;
  }

  @override
  Future<Preferences?> load() async => stored;

  @override
  Future<void> save(Preferences preferences) async {
    saveCount++;
    stored = preferences;
  }
}

class InMemoryFormSnapshotRepository implements FormSnapshotRepository {
  final Map<String, FormSnapshot> byUrl = <String, FormSnapshot>{};
  String? boundSpaceId;

  @override
  Future<void> bindSpace(String spaceId) async {
    boundSpaceId = spaceId;
  }

  @override
  Future<FormSnapshot?> load(String pageUrl) async => byUrl[pageUrl];

  @override
  Future<void> save(FormSnapshot snapshot) async {
    byUrl[snapshot.pageUrl] = snapshot;
  }

  @override
  Future<void> clear(String pageUrl) async {
    byUrl.remove(pageUrl);
  }
}

/// Scriptable [WebViewHost] for controller tests.
class FakeWebViewHost implements WebViewHost {
  final StreamController<WebViewEvent> _events =
      StreamController<WebViewEvent>.broadcast();
  final List<String> loadedUrls = <String>[];
  final List<Map<String, String>> restored = <Map<String, String>>[];
  Map<String, String> formFields = <String, String>{};
  bool disposed = false;

  /// Native history capability of the fake web view. Defaults to false so
  /// tests exercise the plain re-load path unless a test opts in.
  bool canWalkBack = false;
  bool canWalkForward = false;
  int backSteps = 0;
  int forwardSteps = 0;

  @override
  Stream<WebViewEvent> get events => _events.stream;

  void emit(WebViewEvent event) {
    _events.add(event);
  }

  @override
  Future<void> load(String url) async {
    loadedUrls.add(url);
  }

  @override
  Future<void> reload() async {
    loadedUrls.add('reload');
  }

  @override
  Future<bool> canGoBack() async => canWalkBack;

  @override
  Future<bool> canGoForward() async => canWalkForward;

  @override
  Future<void> goBack() async {
    backSteps++;
    loadedUrls.add('goBack');
  }

  @override
  Future<void> goForward() async {
    forwardSteps++;
    loadedUrls.add('goForward');
  }

  @override
  Future<Map<String, String>> captureFormFields() async => formFields;

  @override
  Future<void> restoreFormFields(Map<String, String> fields) async {
    restored.add(fields);
  }

  @override
  void dispose() {
    disposed = true;
    unawaited(_events.close());
  }
}
