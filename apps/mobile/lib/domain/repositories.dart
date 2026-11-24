import 'form_snapshot.dart';
import 'preferences.dart';
import 'tab_stack.dart';

/// Persistence contracts. Implementations bind to exactly one Space before
/// use, and nothing is shared between Spaces.
abstract class TabStackRepository {
  Future<void> bindSpace(String spaceId);
  Future<TabStack?> load();
  Future<void> save(TabStack stack);
  Future<void> clear();
}

abstract class PreferencesRepository {
  Future<void> bindSpace(String spaceId);
  Future<Preferences?> load();
  Future<void> save(Preferences preferences);
}

abstract class FormSnapshotRepository {
  Future<void> bindSpace(String spaceId);
  Future<FormSnapshot?> load(String pageUrl);
  Future<void> save(FormSnapshot snapshot);
  Future<void> clear(String pageUrl);
}
