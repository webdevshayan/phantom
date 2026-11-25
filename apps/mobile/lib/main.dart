import 'package:flutter/material.dart';

import 'infra/persistence/directory_provider.dart';
import 'infra/persistence/hive_form_snapshot_repository.dart';
import 'infra/persistence/hive_preferences_repository.dart';
import 'infra/persistence/hive_store.dart';
import 'infra/persistence/hive_tab_stack_repository.dart';
import 'ui/app_root.dart';
import 'ui/browser/browser_controller.dart';
import 'ui/devtools/devtools_log_store.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Phantom performs zero telemetry: nothing in this entry point or
  // anywhere else in the shell talks to the network on its own. All
  // storage stays inside the Space's own data directory.
  final store = HiveStore(const PathDirectoryProvider());
  final controller = BrowserController(
    tabStackRepository: HiveTabStackRepository(store),
    preferencesRepository: HivePreferencesRepository(store),
    formSnapshotRepository: HiveFormSnapshotRepository(store),
    devToolsLogStore: DevToolsLogStore(),
  );
  runApp(PhantomApp(controller: controller));
}
