import 'dart:io';

import 'package:path_provider/path_provider.dart';

/// Supplies the root directory that holds every Space data directory.
abstract class DirectoryProvider {
  Future<Directory> dataRoot();
}

/// Production provider rooted under the platform's app-support directory.
class PathDirectoryProvider implements DirectoryProvider {
  const PathDirectoryProvider();

  @override
  Future<Directory> dataRoot() async {
    final support = await getApplicationSupportDirectory();
    return Directory('${support.path}/phantom');
  }
}
