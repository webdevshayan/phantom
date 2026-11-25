import 'package:flutter/material.dart';

import '../../domain/tab_entry.dart';
import 'browser_controller.dart';

/// The top address bar. Mirrors the active tab's location and forwards raw
/// input to the controller; resolution lives in the domain layer.
class OmniboxBar extends StatefulWidget {
  final BrowserController controller;

  const OmniboxBar({super.key, required this.controller});

  @override
  State<OmniboxBar> createState() => _OmniboxBarState();
}

class _OmniboxBarState extends State<OmniboxBar> {
  final TextEditingController _text = TextEditingController();
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_syncFromModel);
    _syncFromModel();
  }

  void _syncFromModel() {
    if (_focus.hasFocus) {
      return;
    }
    final url = widget.controller.activeTab.url;
    final display = url == TabEntry.blankUrl ? '' : url;
    if (_text.text != display) {
      _text.text = display;
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_syncFromModel);
    _text.dispose();
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 4),
      child: Container(
        decoration: BoxDecoration(
          color: scheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(24),
        ),
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: TextField(
          controller: _text,
          focusNode: _focus,
          keyboardType: TextInputType.url,
          textInputAction: TextInputAction.go,
          decoration: const InputDecoration(
            hintText: 'Search or type URL',
            icon: Icon(Icons.search),
            border: InputBorder.none,
          ),
          onSubmitted: (value) {
            _focus.unfocus();
            widget.controller.navigate(value);
          },
        ),
      ),
    );
  }
}
