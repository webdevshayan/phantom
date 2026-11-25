import 'package:flutter_test/flutter_test.dart';

import 'package:phantom_mobile/domain/search_engine.dart';

void main() {
  group('SearchEngine', () {
    test('google builds an encoded query url', () {
      final url = SearchEngine.google.buildQueryUrl('phantom browser');
      expect(url, 'https://www.google.com/search?q=phantom%20browser');
    });

    test('duckduckgo builds an encoded query url', () {
      final url = SearchEngine.duckDuckGo.buildQueryUrl('cats & dogs');
      expect(url, 'https://duckduckgo.com/?q=cats%20%26%20dogs');
    });

    test('bing builds an encoded query url', () {
      final url = SearchEngine.bing.buildQueryUrl('flutter');
      expect(url, 'https://www.bing.com/search?q=flutter');
    });

    test('startpage builds an encoded query url', () {
      final url = SearchEngine.startpage.buildQueryUrl('hive');
      expect(url, 'https://www.startpage.com/sp/search?query=hive');
    });

    test('catalog ids are unique and byId round-trips', () {
      final ids = SearchEngine.catalog.map((e) => e.id).toSet();
      expect(ids.length, SearchEngine.catalog.length);
      for (final engine in SearchEngine.catalog) {
        expect(SearchEngine.byId(engine.id), engine);
      }
    });
  });
}
