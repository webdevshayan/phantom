import 'package:flutter/material.dart';

import '../../domain/preferences.dart';

class PhantomTheme {
  const PhantomTheme._();

  static ThemeData light() => ThemeData(
        useMaterial3: true,
        brightness: Brightness.light,
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF3C5AA6)),
      );

  static ThemeData dark() => ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF8AB4F8),
          brightness: Brightness.dark,
        ),
      );

  /// Maps the stored preference onto a concrete mode. The follow-device
  /// option reads the platform brightness directly.
  static ThemeMode resolve(
    ThemePreference preference,
    Brightness platformBrightness,
  ) {
    switch (preference) {
      case ThemePreference.light:
        return ThemeMode.light;
      case ThemePreference.dark:
        return ThemeMode.dark;
      case ThemePreference.followDevice:
        return platformBrightness == Brightness.dark
            ? ThemeMode.dark
            : ThemeMode.light;
    }
  }
}
