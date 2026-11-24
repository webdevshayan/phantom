import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/form_snapshot.dart';

void main() {
  group('FormSnapshot', () {
    test('json round trip preserves the fields', () {
      const snapshot = FormSnapshot(
        pageUrl: 'https://example.com/new-post',
        fields: <String, String>{'title': 'hello', 'body': 'world'},
      );
      final restored = FormSnapshot.fromJson(snapshot.toJson());
      expect(restored.pageUrl, 'https://example.com/new-post');
      expect(restored.fields['title'], 'hello');
      expect(restored.fields['body'], 'world');
    });

    test('corrupt json falls back to empty fields', () {
      final restored = FormSnapshot.fromJson(const <String, dynamic>{
        'pageUrl': 42,
        'fields': 'nope',
      });
      expect(restored.pageUrl, '');
      expect(restored.fields, isEmpty);
    });
  });
}
