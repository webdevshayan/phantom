import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:phantom_mobile/domain/preferences.dart';
import 'package:phantom_mobile/ui/theme/phantom_theme.dart';

void main() {
  group('PhantomTheme.resolve', () {
    test('light stays light', () {
      expect(
        PhantomTheme.resolve(ThemePreference.light, Brightness.dark),
        ThemeMode.light,
      );
    });

    test('dark stays dark', () {
      expect(
        PhantomTheme.resolve(ThemePreference.dark, Brightness.light),
        ThemeMode.dark,
      );
    });

    test('follow device reads the platform brightness', () {
      expect(
        PhantomTheme.resolve(ThemePreference.followDevice, Brightness.dark),
        ThemeMode.dark,
      );
      expect(
        PhantomTheme.resolve(ThemePreference.followDevice, Brightness.light),
        ThemeMode.light,
      );
    });

    test('both themes are available', () {
      expect(PhantomTheme.light().brightness, Brightness.light);
      expect(PhantomTheme.dark().brightness, Brightness.dark);
    });
  });
}
