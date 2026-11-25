import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// The main manifest is the only one that ships in release APKs. Without the
/// INTERNET permission the WebView cannot reach the network and every page
/// load fails with net::ERR_CACHE_MISS.
void main() {
  test('main manifest grants the INTERNET permission', () {
    final manifest =
        File('android/app/src/main/AndroidManifest.xml').readAsStringSync();
    expect(
      manifest,
      contains('android.permission.INTERNET'),
      reason: 'Without INTERNET the WebView fails every load with '
          'net::ERR_CACHE_MISS.',
    );
  });
}
