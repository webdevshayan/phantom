import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/ad_blocker.dart';

void main() {
  group('AdBlockRules', () {
    test('denies known ad hosts', () {
      expect(AdBlockRules.blocksHost('doubleclick.net'), isTrue);
      expect(AdBlockRules.blocksHost('ads.doubleclick.net'), isTrue);
      expect(AdBlockRules.blocksHost('WWW.TABOOLA.COM'), isTrue);
    });

    test('allows unrelated hosts', () {
      expect(AdBlockRules.blocksHost('example.com'), isFalse);
      expect(AdBlockRules.blocksHost('flutter.dev'), isFalse);
    });

    test('suffix matching cannot be spoofed from the front', () {
      expect(AdBlockRules.blocksHost('doubleclick.net.evil.example'), isFalse);
      expect(AdBlockRules.blocksHost('notdoubleclick.net'), isFalse);
    });

    test('empty and unparseable hosts are allowed', () {
      expect(AdBlockRules.blocksHost(''), isFalse);
      expect(AdBlockRules.blocksUrl('not a url'), isFalse);
    });

    test('blocksUrl reads the host of a full url', () {
      expect(
        AdBlockRules.blocksUrl('https://ad.doubleclick.net/pixel'),
        isTrue,
      );
      expect(AdBlockRules.blocksUrl('https://example.com/pixel'), isFalse);
    });
  });
}
