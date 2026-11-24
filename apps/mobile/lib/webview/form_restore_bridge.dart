import 'dart:convert';

/// Builds the JavaScript that re-applies saved form drafts after a page
/// load. Values are only applied to matching, safe field types on the
/// page that saved them.
class FormRestoreBridge {
  const FormRestoreBridge._();

  static String build(Map<String, String> fields) {
    final encoded = jsonEncode(fields);
    return '''
(function () {
  var values = $encoded;
  var nodes = document.querySelectorAll('input, textarea');
  for (var i = 0; i < nodes.length; i++) {
    var node = nodes[i];
    var type = (node.getAttribute('type') || 'text').toLowerCase();
    if (type === 'password' || type === 'hidden' || type === 'file' || type === 'checkbox' || type === 'radio') { continue; }
    var key = node.name || node.id || ('field-' + i);
    if (Object.prototype.hasOwnProperty.call(values, key)) {
      node.value = String(values[key]);
    }
  }
})();
''';
  }
}
