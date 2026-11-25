import 'package:flutter/foundation.dart';

import 'search_engine.dart';

enum OmniboxKind { url, search }

/// What the omnibox input resolved to: either a direct location or a
/// search query built with the configured engine.
@immutable
class OmniboxTarget {
  final OmniboxKind kind;
  final String url;
  final String rawInput;

  const OmniboxTarget.url(this.url, this.rawInput) : kind = OmniboxKind.url;

  const OmniboxTarget.search(this.url, this.rawInput)
      : kind = OmniboxKind.search;
}

/// Turns whatever the user typed into a loadable location. Inputs with a
/// scheme pass through untouched, host-like inputs get an https scheme,
/// and everything else becomes a query for the configured engine.
OmniboxTarget resolveOmniboxInput(String rawInput, SearchEngine engine) {
  final trimmed = rawInput.trim();
  if (trimmed.isEmpty) {
    return OmniboxTarget.url(engine.homeUrl, trimmed);
  }
  final parsed = Uri.tryParse(trimmed);
  final hasScheme =
      parsed != null && (parsed.scheme == 'http' || parsed.scheme == 'https');
  if (hasScheme) {
    return OmniboxTarget.url(trimmed, trimmed);
  }
  if (_looksLikeHost(trimmed)) {
    return OmniboxTarget.url('https://$trimmed', trimmed);
  }
  return OmniboxTarget.search(engine.buildQueryUrl(trimmed), trimmed);
}

bool _looksLikeHost(String input) {
  if (input.contains(' ')) {
    return false;
  }
  final host = input.split('/').first;
  if (host.startsWith('localhost')) {
    return true;
  }
  if (!host.contains('.')) {
    return false;
  }
  final labels = host.split('.');
  if (labels.any((label) => label.isEmpty)) {
    return false;
  }
  final tld = labels.last;
  return tld.length >= 2 && RegExp(r'^[a-zA-Z]+$').hasMatch(tld);
}
