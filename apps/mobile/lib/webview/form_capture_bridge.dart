/// Builds the JavaScript that collects current form field values from the
/// page. Secret inputs are excluded here and again on the Dart side, so
/// saved drafts only ever contain safe field types.
class FormCaptureBridge {
  const FormCaptureBridge._();

  static String build() => '''
(function () {
  var fields = {};
  var nodes = document.querySelectorAll('input, textarea');
  for (var i = 0; i < nodes.length; i++) {
    var node = nodes[i];
    var type = (node.getAttribute('type') || 'text').toLowerCase();
    if (type === 'password' || type === 'hidden' || type === 'file' || type === 'checkbox' || type === 'radio') { continue; }
    if (!node.value) { continue; }
    var key = node.name || node.id || ('field-' + i);
    fields[key] = String(node.value);
  }
  return JSON.stringify({ url: location.href, fields: fields });
})();
''';
}
