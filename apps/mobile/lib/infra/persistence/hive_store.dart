import 'dart:io';

import 'package:hive/hive.dart';

import '../../domain/space.dart';
import 'directory_provider.dart';

/// Binds Hive to one Space's own data directory at a time. Every Space
/// switch closes all open boxes and re-points Hive at the new directory,
/// so a Space can never read boxes that belong to another Space.
class HiveStore {
  final DirectoryProvider directoryProvider;
  Directory? _boundDirectory;
  String? _boundSpaceId;

  HiveStore(this.directoryProvider);

  Directory? get boundDirectory => _boundDirectory;

  String? get boundSpaceId => _boundSpaceId;

  Future<Directory> useSpace(Space space) async {
    if (_boundSpaceId == space.id && _boundDirectory != null) {
      return _boundDirectory!;
    }
    // Hive.close() closes every open box, guaranteeing a clean boundary.
    await Hive.close();
    final root = await directoryProvider.dataRoot();
    final directory =
        Directory('${root.path}/spaces/${space.directorySegment}');
    await directory.create(recursive: true);
    Hive.init(directory.path);
    _boundDirectory = directory;
    _boundSpaceId = space.id;
    return directory;
  }

  Future<Box<E>> openBox<E>(String name) => Hive.openBox<E>(name);
}
