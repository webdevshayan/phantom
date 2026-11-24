/// A Space is an isolated container for browsing data. Every Space owns a
/// dedicated data directory on disk and nothing is ever shared between
/// Spaces: tab stacks, preferences and form drafts all live inside the
/// Space that produced them.
class Space {
  final String id;
  final String label;

  const Space({required this.id, required this.label});

  /// The Space the mobile shell runs in for this session.
  static const Space session = Space(id: 'session', label: 'Session');

  /// Path segment used for this Space's data directory. It is a
  /// plain, sanitized string so directory boundaries cannot be escaped.
  String get directorySegment => id;

  @override
  bool operator ==(Object other) => other is Space && other.id == id;

  @override
  int get hashCode => id.hashCode;

  @override
  String toString() => 'Space($id)';
}
