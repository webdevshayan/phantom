import 'package:flutter/material.dart';

import 'browser/browser_shell.dart';
import 'browser/browser_controller.dart';
import 'theme/phantom_theme.dart';

/// Composition root. Applies the stored theme preference on top of the
/// light and dark themes and mounts the browser shell.
class PhantomApp extends StatelessWidget {
  final BrowserController controller;

  const PhantomApp({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final platformBrightness =
            WidgetsBinding.instance.platformDispatcher.platformBrightness;
        return MaterialApp(
          title: 'Phantom',
          debugShowCheckedModeBanner: false,
          theme: PhantomTheme.light(),
          darkTheme: PhantomTheme.dark(),
          themeMode: PhantomTheme.resolve(
            controller.preferences.theme,
            platformBrightness,
          ),
          home: BrowserShell(controller: controller),
        );
      },
    );
  }
}
