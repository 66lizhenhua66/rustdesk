import 'dart:ffi' show DynamicLibrary;
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/controller_input/controller_input_engine.dart';
import 'package:flutter_hbb/controller_input/flutter_input_adapter.dart';

void main() {
  late DynamicLibrary library;

  setUpAll(() {
    final path = Platform.environment['CONTROLLER_INPUT_LIBRARY'];
    if (path == null || path.isEmpty) {
      fail(
          'Set CONTROLLER_INPUT_LIBRARY to the built remote_controller_core DLL.');
    }
    library = DynamicLibrary.open(path);
  });

  test('real engine turns touch taps and drags into remote commands', () {
    final commands = <Map<String, dynamic>>[];
    final engine = ControllerInputEngine(library, (command) {
      commands.add(command);
      return 0;
    }, onFailure: (status) => fail('Unexpected input failure: $status'));
    addTearDown(engine.dispose);
    const start = ControllerInputPoint(7, 50, 25);
    const end = ControllerInputPoint(7, 90, 50);
    int touch(ControllerTouchAction action, ControllerInputPoint point,
            int time) =>
        engine.touch(
          action: action,
          points: action == ControllerTouchAction.up ? [] : [point],
          changedPoints: [point],
          time: time,
          width: 200,
          height: 100,
        );

    expect(touch(ControllerTouchAction.down, start, 0), 0);
    expect(touch(ControllerTouchAction.up, start, 20), 0);
    expect(commands.first, {'kind': 'move', 'x': 16384, 'y': 16384});
    expect(commands.where((command) => command['kind'] == 'button'), [
      {'kind': 'button', 'button': 'left', 'down': true},
      {'kind': 'button', 'button': 'left', 'down': false},
    ]);

    commands.clear();
    expect(touch(ControllerTouchAction.down, start, 30), 0);
    expect(touch(ControllerTouchAction.move, end, 40), 0);
    expect(touch(ControllerTouchAction.up, end, 50), 0);
    final down = commands.indexWhere((command) => command['down'] == true);
    expect(down, greaterThanOrEqualTo(0));
    expect(commands[down + 1], {'kind': 'move', 'x': 29491, 'y': 32768});
    expect(commands.last, {'kind': 'button', 'button': 'left', 'down': false});
  });

  test('real engine releases keys and stops a failing synchronous sink', () {
    final commands = <Map<String, dynamic>>[];
    var rejectButton = false;
    var throwInSink = false;
    final failures = <int>[];
    final engine = ControllerInputEngine(library, (command) {
      commands.add(command);
      if (throwInSink) throw StateError('sink unavailable');
      return rejectButton && command['kind'] == 'button' ? 4 : 0;
    }, onFailure: failures.add);
    addTearDown(engine.dispose);
    expect(engine.key(down: true, physicalCode: 0x70004, code: 'KeyA'), 0);
    expect(engine.release(), 0);
    expect(commands, [
      {'kind': 'key', 'code': 'KeyA', 'down': true},
      {'kind': 'release_all'},
    ]);
    expect(engine.key(down: true, physicalCode: 0x70006, code: 'KeyC'), 0);
    commands.clear();
    expect(engine.commitText(List.filled(8193, 'a').join()), isNot(0));
    expect(engine.key(down: false, physicalCode: 0x70006, code: 'KeyC'), 0);
    expect(commands, isEmpty);
    commands.clear();
    rejectButton = true;
    const point = ControllerInputPoint(1, 10, 10);
    expect(
        engine.touch(
            action: ControllerTouchAction.down,
            points: [point],
            changedPoints: [point],
            time: 0,
            width: 100,
            height: 100),
        0);
    expect(
        engine.touch(
            action: ControllerTouchAction.up,
            points: [],
            changedPoints: [point],
            time: 20,
            width: 100,
            height: 100),
        isNot(0));
    expect(commands.where((command) => command['kind'] == 'button'), [
      {'kind': 'button', 'button': 'left', 'down': true},
    ]);
    commands.clear();
    expect(engine.release(), 0);
    expect(commands, isEmpty);
    throwInSink = true;
    expect(
        engine.key(down: true, physicalCode: 0x70005, code: 'KeyB'), isNot(0));
    expect(engine.sinkError, isA<StateError>());
    expect(failures, hasLength(3));
    engine.reset();
    engine.dispose();
    expect(engine.release, throwsStateError);
  });

  test('Flutter adapter maps touch sets, button transitions, keys and IME', () {
    final commands = <Map<String, dynamic>>[];
    final engine = ControllerInputEngine(library, (command) {
      commands.add(command);
      return 0;
    }, onFailure: (status) => fail('Unexpected input failure: $status'));
    addTearDown(engine.dispose);
    final adapter = FlutterInputAdapter(engine, viewport: const Size(200, 100));
    adapter.pointer(const PointerDownEvent(
        pointer: 8, kind: PointerDeviceKind.touch, position: Offset(30, 20)));
    adapter.pointer(const PointerDownEvent(
        pointer: 9, kind: PointerDeviceKind.touch, position: Offset(70, 20)));
    adapter.pointer(const PointerMoveEvent(
        pointer: 8, kind: PointerDeviceKind.touch, position: Offset(30, 68)));
    expect(commands.where((command) => command['kind'] == 'wheel'), [
      {'kind': 'wheel', 'dx': 0, 'dy': 1},
    ]);
    adapter.pointer(
        const PointerCancelEvent(pointer: 8, kind: PointerDeviceKind.touch));
    commands.clear();
    adapter.pointer(const PointerDownEvent(
        kind: PointerDeviceKind.mouse,
        position: Offset(50, 25),
        buttons: kPrimaryMouseButton));
    adapter.pointer(const PointerMoveEvent(
        kind: PointerDeviceKind.mouse,
        position: Offset(50, 25),
        buttons: kPrimaryMouseButton | kSecondaryMouseButton));
    adapter.pointer(const PointerMoveEvent(
        kind: PointerDeviceKind.mouse,
        position: Offset(50, 25),
        buttons: kSecondaryMouseButton));
    adapter.pointer(const PointerUpEvent(
        kind: PointerDeviceKind.mouse, position: Offset(50, 25)));
    expect(commands.where((command) => command['kind'] == 'button'), [
      {'kind': 'button', 'button': 'left', 'down': true},
      {'kind': 'button', 'button': 'right', 'down': true},
      {'kind': 'button', 'button': 'left', 'down': false},
      {'kind': 'button', 'button': 'right', 'down': false},
    ]);
    adapter.pointerSignal(const PointerScrollEvent(
        kind: PointerDeviceKind.mouse,
        position: Offset(50, 25),
        scrollDelta: Offset(40, 40)));
    expect(commands.last, {'kind': 'wheel', 'dx': 1, 'dy': -1});
    expect(
        adapter.key(const KeyDownEvent(
            physicalKey: PhysicalKeyboardKey.keyA,
            logicalKey: LogicalKeyboardKey.keyQ,
            timeStamp: Duration.zero)),
        0);
    expect(commands.last, {'kind': 'key', 'code': 'KeyQ', 'down': true});
    expect(adapter.focusChanged(false), 0);
    expect(commands.last, {'kind': 'release_all'});
    expect(adapter.commitText('中文'), 0);
    expect(commands.last, {'kind': 'text', 'text': '中文'});
  });
}
