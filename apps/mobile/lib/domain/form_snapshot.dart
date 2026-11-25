import 'package:flutter/foundation.dart';

/// Draft text captured from a page's form fields. Password, hidden and
/// file inputs are never captured, and the values only ever live inside
/// the Space's own data directory.
@immutable
class FormSnapshot {
  final String pageUrl;
  final Map<String, String> fields;

  const FormSnapshot({required this.pageUrl, required this.fields});

  Map<String, dynamic> toJson() => <String, dynamic>{
        'pageUrl': pageUrl,
        'fields': fields,
      };

  factory FormSnapshot.fromJson(Map<String, dynamic> json) {
    final pageUrl = json['pageUrl'];
    final rawFields = json['fields'];
    return FormSnapshot(
      pageUrl: pageUrl is String ? pageUrl : '',
      fields: rawFields is Map
          ? Map<String, String>.from(rawFields)
          : const <String, String>{},
    );
  }
}
