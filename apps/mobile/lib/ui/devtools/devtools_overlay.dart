import 'package:flutter/material.dart';

import 'devtools_log_store.dart';

void showDevToolsOverlay(BuildContext context, DevToolsLogStore store) {
  showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (sheetContext) => FractionallySizedBox(
      heightFactor: 0.85,
      child: DevToolsOverlay(store: store),
    ),
  );
}

/// Developer tools overlay: a live feed of console output and network
/// activity captured through the injected bridge.
class DevToolsOverlay extends StatelessWidget {
  final DevToolsLogStore store;

  const DevToolsOverlay({super.key, required this.store});

  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: const Text('Developer tools'),
          actions: <Widget>[
            IconButton(
              tooltip: 'Clear',
              icon: const Icon(Icons.delete_outline),
              onPressed: store.clear,
            ),
          ],
          bottom: const TabBar(
            tabs: <Widget>[
              Tab(text: 'Console'),
              Tab(text: 'Network'),
            ],
          ),
        ),
        body: TabBarView(
          children: <Widget>[
            _ConsoleList(store: store),
            _NetworkList(store: store),
          ],
        ),
      ),
    );
  }
}

class _ConsoleList extends StatelessWidget {
  final DevToolsLogStore store;

  const _ConsoleList({required this.store});

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: store,
      builder: (context, _) {
        final entries = store.console;
        if (entries.isEmpty) {
          return const _EmptyState(message: 'No console entries yet');
        }
        return ListView.separated(
          itemCount: entries.length,
          separatorBuilder: (_, __) => const Divider(height: 1),
          itemBuilder: (context, index) {
            final entry = entries[index];
            return ListTile(
              dense: true,
              leading: Icon(
                _levelIcon(entry.level),
                color: _levelColor(context, entry.level),
                size: 18,
              ),
              title: Text(
                entry.message,
                style: Theme.of(context).textTheme.bodySmall,
              ),
              subtitle: Text(
                '${entry.level} · ${entry.at.toIso8601String()}',
                style: Theme.of(context).textTheme.labelSmall,
              ),
            );
          },
        );
      },
    );
  }

  IconData _levelIcon(String level) {
    switch (level) {
      case 'error':
        return Icons.error_outline;
      case 'warn':
        return Icons.warning_amber_outlined;
      case 'info':
        return Icons.info_outline;
      default:
        return Icons.chevron_right;
    }
  }

  Color? _levelColor(BuildContext context, String level) {
    switch (level) {
      case 'error':
        return Theme.of(context).colorScheme.error;
      case 'warn':
        return const Color(0xFFB26A00);
      default:
        return null;
    }
  }
}

class _NetworkList extends StatelessWidget {
  final DevToolsLogStore store;

  const _NetworkList({required this.store});

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: store,
      builder: (context, _) {
        final entries = store.network;
        if (entries.isEmpty) {
          return const _EmptyState(message: 'No network entries yet');
        }
        return ListView.separated(
          itemCount: entries.length,
          separatorBuilder: (_, __) => const Divider(height: 1),
          itemBuilder: (context, index) {
            final entry = entries[index];
            return ListTile(
              dense: true,
              leading: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 6,
                  vertical: 2,
                ),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Text(
                  entry.method,
                  style: Theme.of(context).textTheme.labelSmall,
                ),
              ),
              title: Text(
                entry.url,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.bodySmall,
              ),
              subtitle: Text(
                '${entry.status} · ${entry.durationMs} ms · ${entry.at.toIso8601String()}',
                style: Theme.of(context).textTheme.labelSmall,
              ),
            );
          },
        );
      },
    );
  }
}

class _EmptyState extends StatelessWidget {
  final String message;

  const _EmptyState({required this.message});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        message,
        style: Theme.of(context).textTheme.bodyMedium,
      ),
    );
  }
}
