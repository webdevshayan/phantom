import 'package:flutter/material.dart';

import 'browser_controller.dart';

/// Full-screen list of open tabs with drag-to-reorder, close buttons and
/// a new-tab action.
class TabSwitcherPage extends StatelessWidget {
  final BrowserController controller;

  const TabSwitcherPage({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Tabs')),
      body: AnimatedBuilder(
        animation: controller,
        builder: (context, _) {
          final tabs = controller.tabs;
          return ReorderableListView.builder(
            itemCount: tabs.length,
            onReorder: (oldIndex, newIndex) {
              controller.reorderTabs(oldIndex, newIndex);
            },
            itemBuilder: (context, index) {
              final tab = tabs[index];
              final isActive = tab.id == controller.activeTab.id;
              return ListTile(
                key: ValueKey<String>(tab.id),
                leading: Icon(
                  isActive ? Icons.check_circle : Icons.radio_button_unchecked,
                  color:
                      isActive ? Theme.of(context).colorScheme.primary : null,
                ),
                title: Text(
                  tab.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: Text(
                  tab.url,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                trailing: IconButton(
                  icon: const Icon(Icons.close),
                  tooltip: 'Close tab',
                  onPressed: () {
                    controller.closeTab(tab.id);
                  },
                ),
                onTap: () {
                  Navigator.of(context).pop();
                  controller.selectTab(tab.id);
                },
              );
            },
          );
        },
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () {
          Navigator.of(context).pop();
          controller.addTab();
        },
        icon: const Icon(Icons.add),
        label: const Text('New tab'),
      ),
    );
  }
}
