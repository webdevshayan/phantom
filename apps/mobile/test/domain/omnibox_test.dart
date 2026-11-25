import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/omnibox.dart';
import 'package:phantom_mobile/domain/search_engine.dart';

void main() {
  const engine = SearchEngine.google;

  group('resolveOmniboxInput', () {
    test('empty input resolves to the engine home', () {
      final target = resolveOmniboxInput('   ', engine);
      expect(target.kind, OmniboxKind.url);
      expect(target.url, engine.homeUrl);
    });

    test('full urls pass through untouched', () {
      final target = resolveOmniboxInput('https://example.com/a?b=c', engine);
      expect(target.kind, OmniboxKind.url);
      expect(target.url, 'https://example.com/a?b=c');
    });

    test('host-like input gains an https scheme', () {
      expect(resolveOmniboxInput('example.com', engine).url,
          'https://example.com');
      expect(resolveOmniboxInput('example.com/path', engine).url,
          'https://example.com/path');
      expect(resolveOmniboxInput('localhost:8080', engine).url,
          'https://localhost:8080');
    });

    test('plain phrases become search queries', () {
      final target = resolveOmniboxInput('phantom browser', engine);
      expect(target.kind, OmniboxKind.search);
      expect(target.url, 'https://www.google.com/search?q=phantom%20browser');
    });

    test('every catalog engine can serve a query', () {
      for (final candidate in SearchEngine.catalog) {
        final target = resolveOmniboxInput('cats', candidate);
        expect(target.kind, OmniboxKind.search);
        expect(target.url, contains(candidate.homeUrl.substring(8)));
      }
    });
  });
}
