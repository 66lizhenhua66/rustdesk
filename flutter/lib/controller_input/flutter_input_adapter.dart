import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import 'controller_input_engine.dart';

/// Converts Flutter event envelopes; gesture recognition remains in Rust.
class FlutterInputAdapter {
  FlutterInputAdapter(this.engine, {required Size viewport})
      : _viewport = viewport;

  final ControllerInputEngine engine;
  Size _viewport;
  final _touches = <int, ControllerInputPoint>{};
  int _buttons = 0;

  int pointer(PointerEvent event) {
    if (event is PointerCancelEvent) return release();
    if (event.kind == PointerDeviceKind.touch) return _touch(event);
    if (event is! PointerDownEvent &&
        event is! PointerMoveEvent &&
        event is! PointerUpEvent &&
        event is! PointerHoverEvent) return 0;

    final buttons = event is PointerUpEvent ? 0 : event.buttons;
    var changed = false;
    for (final entry in _mouseButtons.entries) {
      if ((_buttons & entry.key) == (buttons & entry.key)) continue;
      changed = true;
      final status = engine.mouse(
        action: buttons & entry.key != 0
            ? ControllerMouseAction.press
            : ControllerMouseAction.release,
        button: entry.value,
        x: event.localPosition.dx,
        y: event.localPosition.dy,
        width: _viewport.width,
        height: _viewport.height,
      );
      if (status != 0) return _complete(status);
    }
    _buttons = buttons;
    if (changed) return 0;
    return _complete(engine.mouse(
      action: ControllerMouseAction.move,
      x: event.localPosition.dx,
      y: event.localPosition.dy,
      width: _viewport.width,
      height: _viewport.height,
    ));
  }

  int _touch(PointerEvent event) {
    final ControllerTouchAction action;
    if (event is PointerDownEvent) {
      action = ControllerTouchAction.down;
    } else if (event is PointerMoveEvent) {
      action = ControllerTouchAction.move;
    } else if (event is PointerUpEvent) {
      action = ControllerTouchAction.up;
    } else {
      return 0;
    }
    final point = ControllerInputPoint(
        event.pointer, event.localPosition.dx, event.localPosition.dy);
    if (action == ControllerTouchAction.up) {
      _touches.remove(event.pointer);
    } else {
      _touches[event.pointer] = point;
    }
    return _complete(engine.touch(
      action: action,
      points: _touches.values.toList(),
      changedPoints: [point],
      time: event.timeStamp.inMilliseconds,
      width: _viewport.width,
      height: _viewport.height,
    ));
  }

  int pointerSignal(PointerSignalEvent event) {
    if (event is! PointerScrollEvent) return 0;
    return _complete(engine.wheel(
      x: event.localPosition.dx,
      y: event.localPosition.dy,
      dx: event.scrollDelta.dx,
      dy: -event.scrollDelta.dy,
      discrete: false,
      width: _viewport.width,
      height: _viewport.height,
    ));
  }

  int key(KeyEvent event) {
    final code = keyCode(event.physicalKey, event.logicalKey);
    if (code == null) return 0;
    return _complete(engine.key(
      down: event is! KeyUpEvent,
      physicalCode: event.physicalKey.usbHidUsage,
      code: code,
    ));
  }

  int commitText(String text) => _complete(engine.commitText(text));

  int focusChanged(bool focused) => focused ? 0 : release();

  int updateViewport(Size viewport) {
    if (viewport == _viewport) return 0;
    final status = release();
    _viewport = viewport;
    return status;
  }

  int release() {
    _touches.clear();
    _buttons = 0;
    return engine.release();
  }

  /// Use only after the session has disabled input or disconnected.
  void reset() {
    _touches.clear();
    _buttons = 0;
    engine.reset();
  }

  int _complete(int status) {
    if (status != 0) {
      _touches.clear();
      _buttons = 0;
    }
    return status;
  }

  static const _mouseButtons = {
    kPrimaryMouseButton: ControllerMouseButton.left,
    kSecondaryMouseButton: ControllerMouseButton.right,
    kMiddleMouseButton: ControllerMouseButton.middle,
  };

  static String? keyCode(
      PhysicalKeyboardKey physical, LogicalKeyboardKey logical) {
    final usage = physical.usbHidUsage;
    if (usage >= 0x70059 && usage <= 0x70061) {
      return 'Numpad${usage - 0x70058}';
    }
    if (usage == 0x70062) return 'Numpad0';
    final numpad = _numpadKeys[physical];
    if (numpad != null) return numpad;
    final id = logical.keyId;
    if (id >= 0x61 && id <= 0x7a) {
      return 'Key${String.fromCharCode(id).toUpperCase()}';
    }
    if (id >= 0x30 && id <= 0x39) return 'Digit${id - 0x30}';
    if (id >= LogicalKeyboardKey.f1.keyId &&
        id <= LogicalKeyboardKey.f12.keyId) {
      return 'F${id - LogicalKeyboardKey.f1.keyId + 1}';
    }
    return _logicalKeys[logical];
  }

  static final _numpadKeys = {
    PhysicalKeyboardKey.numpadAdd: 'NumpadAdd',
    PhysicalKeyboardKey.numpadSubtract: 'NumpadSubtract',
    PhysicalKeyboardKey.numpadMultiply: 'NumpadMultiply',
    PhysicalKeyboardKey.numpadDivide: 'NumpadDivide',
    PhysicalKeyboardKey.numpadDecimal: 'NumpadDecimal',
    PhysicalKeyboardKey.numpadEnter: 'NumpadEnter',
  };

  static final _logicalKeys = {
    LogicalKeyboardKey.enter: 'Enter',
    LogicalKeyboardKey.escape: 'Escape',
    LogicalKeyboardKey.tab: 'Tab',
    LogicalKeyboardKey.space: 'Space',
    LogicalKeyboardKey.backspace: 'Backspace',
    LogicalKeyboardKey.delete: 'Delete',
    LogicalKeyboardKey.arrowLeft: 'ArrowLeft',
    LogicalKeyboardKey.arrowRight: 'ArrowRight',
    LogicalKeyboardKey.arrowUp: 'ArrowUp',
    LogicalKeyboardKey.arrowDown: 'ArrowDown',
    LogicalKeyboardKey.home: 'Home',
    LogicalKeyboardKey.end: 'End',
    LogicalKeyboardKey.pageUp: 'PageUp',
    LogicalKeyboardKey.pageDown: 'PageDown',
    LogicalKeyboardKey.shiftLeft: 'Shift',
    LogicalKeyboardKey.shiftRight: 'Shift',
    LogicalKeyboardKey.controlLeft: 'Control',
    LogicalKeyboardKey.controlRight: 'Control',
    LogicalKeyboardKey.altLeft: 'Alt',
    LogicalKeyboardKey.altRight: 'Alt',
    LogicalKeyboardKey.metaLeft: 'MetaLeft',
    LogicalKeyboardKey.metaRight: 'MetaRight',
    LogicalKeyboardKey.minus: 'Minus',
    LogicalKeyboardKey.equal: 'Equal',
    LogicalKeyboardKey.bracketLeft: 'BracketLeft',
    LogicalKeyboardKey.bracketRight: 'BracketRight',
    LogicalKeyboardKey.backslash: 'Backslash',
    LogicalKeyboardKey.semicolon: 'Semicolon',
    LogicalKeyboardKey.quote: 'Quote',
    LogicalKeyboardKey.backquote: 'Backquote',
    LogicalKeyboardKey.comma: 'Comma',
    LogicalKeyboardKey.period: 'Period',
    LogicalKeyboardKey.slash: 'Slash',
    LogicalKeyboardKey.capsLock: 'CapsLock',
    LogicalKeyboardKey.insert: 'Insert',
    LogicalKeyboardKey.numLock: 'NumLock',
    LogicalKeyboardKey.scrollLock: 'ScrollLock',
  };
}
