/// Host deny list used when the ad blocker flag is on. Navigations to
/// these hosts are cancelled before the page ever loads. Keeping the list
/// in the domain layer keeps the rule pure and easy to test.
class AdBlockRules {
  static const Set<String> deniedHostSuffixes = <String>{
    'doubleclick.net',
    'googlesyndication.com',
    'googleadservices.com',
    'google-analytics.com',
    'adnxs.com',
    'adsrvr.org',
    'criteo.com',
    'taboola.com',
    'outbrain.com',
    'scorecardresearch.com',
    'quantserve.com',
  };

  static bool blocksHost(String host) {
    final normalized = host.toLowerCase();
    if (normalized.isEmpty) {
      return false;
    }
    for (final suffix in deniedHostSuffixes) {
      if (normalized == suffix || normalized.endsWith('.$suffix')) {
        return true;
      }
    }
    return false;
  }

  static bool blocksUrl(String url) {
    final parsed = Uri.tryParse(url);
    if (parsed == null) {
      return false;
    }
    return blocksHost(parsed.host);
  }

  const AdBlockRules._();
}
