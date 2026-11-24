import 'package:flutter/material.dart';

import 'browser_controller.dart';
import 'browser_menu.dart';
import 'tab_switcher_page.dart';

/// Back / forward / reload on the left, the tab count and the three-dot
/// menu on the right.
class NavigationRow extends StatelessWidget {
  final BrowserController controller;

  const NavigationRow({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: <Widget>[
        IconButton(
          tooltip: 'Back',
          onPressed: controller.activeTab.canGoBack
              ? () {
                  controller.back();
                }
              : null,
          icon: const Icon(Icons.arrow_back),
        ),
        IconButton(
          tooltip: 'Forward',
          onPressed: controller.activeTab.canGoForward
              ? () {
                  controller.forward();
                }
              : null,
          icon: const Icon(Icons.arrow_forward),
        ),
        IconButton(
          tooltip: 'Reload',
          onPressed: () {
            controller.reload();
          },
          icon: const Icon(Icons.refresh),
        ),
        const Spacer(),
        _TabCountBadge(
          count: controller.tabs.length,
          onTap: () => _openTabSwitcher(context),
        ),
        BrowserMenu(controller: controller),
      ],
    );
  }

  void _openTabSwitcher(BuildContext context) {
    Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (_) => TabSwitcherPage(controller: controller),
      ),
    );
  }
}

class _TabCountBadge extends StatelessWidget {
  final int count;
  final VoidCallback onTap;

  const _TabCountBadge({required this.count, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(14),
      child: Tooltip(
        message: 'Tab switcher',
        child: Container(
          width: 28,
          height: 28,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            border: Border.all(color: Theme.of(context).colorScheme.outline),
          ),
          child: Text(
            '$count',
            style: Theme.of(context).textTheme.labelLarge,
          ),
        ),
      ),
    );
  }
}
