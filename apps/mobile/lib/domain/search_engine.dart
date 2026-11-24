import 'package:flutter/foundation.dart';

import 'preferences.dart';

@immutable
class SearchEngine {
  final SearchEngineId id;
  final String label;
  final String queryTemplate;
  final String homeUrl;

  const SearchEngine({
    required this.id,
    required this.label,
    required this.queryTemplate,
    required this.homeUrl,
  });

  String buildQueryUrl(String query) =>
      queryTemplate.replaceFirst('%s', Uri.encodeComponent(query));

  static const SearchEngine google = SearchEngine(
    id: SearchEngineId.google,
    label: 'Google',
    queryTemplate: 'https://www.google.com/search?q=%s',
    homeUrl: 'https://www.google.com',
  );

  static const SearchEngine duckDuckGo = SearchEngine(
    id: SearchEngineId.duckDuckGo,
    label: 'DuckDuckGo',
    queryTemplate: 'https://duckduckgo.com/?q=%s',
    homeUrl: 'https://duckduckgo.com',
  );

  static const SearchEngine bing = SearchEngine(
    id: SearchEngineId.bing,
    label: 'Bing',
    queryTemplate: 'https://www.bing.com/search?q=%s',
    homeUrl: 'https://www.bing.com',
  );

  static const SearchEngine startpage = SearchEngine(
    id: SearchEngineId.startpage,
    label: 'Startpage',
    queryTemplate: 'https://www.startpage.com/sp/search?query=%s',
    homeUrl: 'https://www.startpage.com',
  );

  static const List<SearchEngine> catalog = <SearchEngine>[
    google,
    duckDuckGo,
    bing,
    startpage,
  ];

  static SearchEngine byId(SearchEngineId id) =>
      catalog.firstWhere((engine) => engine.id == id);
}
