import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import 'controller_canvas_engine.dart';
import 'controller_input_engine.dart';
import 'flutter_input_adapter.dart';

/// Converts viewport-local Flutter events; Rust owns canvas and gestures.
class FlutterCanvasInputAdapter {
  FlutterCanvasInputAdapter(this.engine);

  final ControllerCanvasEngine engine;
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
      );
      if (status != 0) return _complete(status);
    }
    _buttons = buttons;
    if (changed) return 0;
    return _complete(engine.mouse(
      action: ControllerMouseAction.move,
      x: event.localPosition.dx,
      y: event.localPosition.dy,
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
    ));
  }

  int key(KeyEvent event) {
    final code =
        FlutterInputAdapter.keyCode(event.physicalKey, event.logicalKey);
    if (code == null) return 0;
    return _complete(engine.key(
      down: event is! KeyUpEvent,
      physicalCode: event.physicalKey.usbHidUsage,
      code: code,
    ));
  }

  int commitText(String text) => _complete(engine.commitText(text));
  int focusChanged(bool focused) => focused ? 0 : release();

  int release() {
    clearLocal();
    return engine.release();
  }

  void clearLocal() {
    _touches.clear();
    _buttons = 0;
  }

  int _complete(int status) {
    if (status != 0) clearLocal();
    return status;
  }

  static const _mouseButtons = {
    kPrimaryMouseButton: ControllerMouseButton.left,
    kSecondaryMouseButton: ControllerMouseButton.right,
    kMiddleMouseButton: ControllerMouseButton.middle,
  };
}
