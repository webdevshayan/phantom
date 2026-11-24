import 'dart:convert';

import 'package:flutter/foundation.dart';

@immutable
class ConsoleEntry {
  final String level;
  final String message;
  final DateTime at;

  const ConsoleEntry({
    required this.level,
    required this.message,
    required this.at,
  });
}

@immutable
class NetworkEntry {
  final String method;
  final String url;
  final int status;
  final int durationMs;
  final DateTime at;

  const NetworkEntry({
    required this.method,
    required this.url,
    required this.status,
    required this.durationMs,
    required this.at,
  });
}

/// Holds the console and network feed shown by the developer tools
/// overlay. Capacity-capped so a chatty page can never balloon memory.
class DevToolsLogStore extends ChangeNotifier {
  final List<ConsoleEntry> console = <ConsoleEntry>[];
  final List<NetworkEntry> network = <NetworkEntry>[];
  final int capacity;

  DevToolsLogStore({this.capacity = 500});

  void handleRaw(String raw) {
    Map<String, dynamic> payload;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) {
        return;
      }
      payload = Map<String, dynamic>.from(decoded);
    } on FormatException {
      return;
    }
    final kind = payload['kind'];
    final at = _timestamp(payload['at']);
    if (kind == 'console') {
      console.add(ConsoleEntry(
        level: payload['level']?.toString() ?? 'log',
        message: payload['message']?.toString() ?? '',
        at: at,
      ));
    } else if (kind == 'network') {
      network.add(NetworkEntry(
        method: payload['method']?.toString() ?? 'GET',
        url: payload['url']?.toString() ?? '',
        status: (payload['status'] as num?)?.toInt() ?? 0,
        durationMs: (payload['durationMs'] as num?)?.toInt() ?? 0,
        at: at,
      ));
    } else {
      return;
    }
    _trim();
    notifyListeners();
  }

  void appendConsole(String level, String message) {
    console.add(ConsoleEntry(
      level: level,
      message: message,
      at: DateTime.now(),
    ));
    _trim();
    notifyListeners();
  }

  void clear() {
    console.clear();
    network.clear();
    notifyListeners();
  }

  DateTime _timestamp(dynamic value) {
    final millis = (value as num?)?.toInt();
    if (millis == null || millis <= 0) {
      return DateTime.now();
    }
    return DateTime.fromMillisecondsSinceEpoch(millis);
  }

  void _trim() {
    while (console.length > capacity) {
      console.removeAt(0);
    }
    while (network.length > capacity) {
      network.removeAt(0);
    }
  }
}
