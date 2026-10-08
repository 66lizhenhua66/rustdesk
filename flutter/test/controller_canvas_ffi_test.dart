import 'dart:ffi' show DynamicLibrary;
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/controller_input/controller_canvas_engine.dart';
import 'package:flutter_hbb/controller_input/controller_canvas_view.dart';
import 'package:flutter_hbb/controller_input/controller_input_engine.dart';

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

  ControllerCanvasEngine canvas(List<Map<String, dynamic>> commands) =>
      ControllerCanvasEngine(library, (command) {
        commands.add(command);
        return 0;
      }, onFailure: (status) => fail('Unexpected canvas failure: $status'));

  void configure(ControllerCanvasEngine engine,
      {double width = 400, double height = 800, bool enabled = true}) {
    expect(
        engine.configure(
          viewportWidth: width,
          viewportHeight: height,
          remoteWidth: 1920,
          remoteHeight: 1080,
          insets: const CanvasInsets(top: 20, bottom: 30),
          enabled: enabled,
        ),
        0);
  }

  int touch(
          ControllerCanvasEngine engine,
          ControllerTouchAction action,
          List<ControllerInputPoint> points,
          ControllerInputPoint changed,
          int time) =>
      engine.touch(
        action: action,
        points: points,
        changedPoints: [changed],
        time: time,
      );

  test(
      'real canvas fits, zooms to 10x, preserves center on rotation and resets',
      () {
    final commands = <Map<String, dynamic>>[];
    final engine = canvas(commands);
    addTearDown(engine.dispose);
    configure(engine);
    final fitted = engine.state;
    expect(fitted.zoom, 1);
    expect(fitted.imageWidth, closeTo(352, 0.01));
    expect(fitted.imageHeight, closeTo(198, 0.01));
    expect(fitted.safeY, 44);
    expect(fitted.safeHeight, 702);
    expect(fitted.resetVisible, false);

    const a = ControllerInputPoint(1, 150, 300);
    var b = const ControllerInputPoint(2, 250, 300);
    touch(engine, ControllerTouchAction.down, [a], a, 0);
    touch(engine, ControllerTouchAction.down, [a, b], b, 10);
    b = const ControllerInputPoint(2, 1150, 300);
    touch(engine, ControllerTouchAction.move, [a, b], b, 30);
    expect(engine.state.zoom, 10);
    expect(engine.state.resetVisible, true);
    expect(commands.where((c) => c['kind'] == 'button'), isEmpty);
    touch(engine, ControllerTouchAction.up, [a], b, 40);
    touch(engine, ControllerTouchAction.up, [], a, 50);

    final old = engine.state;
    final oldCenterX =
        (old.safeX + old.safeWidth / 2 - old.imageX) / (old.imageWidth / 1920);
    configure(engine, width: 800, height: 400);
    final rotated = engine.state;
    final newCenterX =
        (rotated.safeX + rotated.safeWidth / 2 - rotated.imageX) /
            (rotated.imageWidth / 1920);
    expect(rotated.zoom, 10);
    expect(newCenterX, closeTo(oldCenterX, 1));
    expect(engine.resetView(), 0);
    expect(engine.state.zoom, 1);
    expect(engine.state.resetVisible, false);
  });

  test('real canvas maps taps and delayed drag; readonly still pans locally',
      () {
    final commands = <Map<String, dynamic>>[];
    final engine = canvas(commands);
    addTearDown(engine.dispose);
    configure(engine);
    final state = engine.state;
    final center = ControllerInputPoint(1, state.imageX + state.imageWidth / 2,
        state.imageY + state.imageHeight / 2);
    touch(engine, ControllerTouchAction.down, [center], center, 0);
    expect(engine.state.nextTickMs, 1000);
    touch(engine, ControllerTouchAction.up, [], center, 20);
    expect(commands.first, {'kind': 'move', 'x': 32768, 'y': 32768});
    expect(commands.where((c) => c['kind'] == 'button').length, 2);

    commands.clear();
    touch(engine, ControllerTouchAction.down, [center], center, 100);
    expect(engine.tick(1100), 0);
    expect(engine.state.dragReady, true);
    final moved = ControllerInputPoint(1, center.x + 20, center.y);
    touch(engine, ControllerTouchAction.move, [moved], moved, 1120);
    touch(engine, ControllerTouchAction.up, [], moved, 1140);
    expect(commands.where((c) => c['kind'] == 'button').map((c) => c['down']),
        [true, false]);

    commands.clear();
    configure(engine, enabled: false);
    expect(engine.state.inputEnabled, false);
    final a = ControllerInputPoint(1, center.x - 50, center.y);
    var b = ControllerInputPoint(2, center.x + 50, center.y);
    touch(engine, ControllerTouchAction.down, [a], a, 2000);
    touch(engine, ControllerTouchAction.down, [a, b], b, 2010);
    b = ControllerInputPoint(2, center.x + 150, center.y);
    touch(engine, ControllerTouchAction.move, [a, b], b, 2030);
    expect(engine.state.zoom, greaterThan(1));
    expect(commands, isEmpty);

    expect(engine.release(), 0);
    expect(engine.setTouchMode(CanvasTouchMode.pointer), 0);
    final beforePan = engine.state.imageX;
    final panStart = ControllerInputPoint(3, center.x, center.y);
    final panEnd = ControllerInputPoint(3, center.x + 24, center.y);
    touch(engine, ControllerTouchAction.down, [panStart], panStart, 3000);
    touch(engine, ControllerTouchAction.move, [panEnd], panEnd, 3020);
    expect(engine.state.imageX, isNot(beforePan));
    expect(commands, isEmpty);
  });

  test('canvas sink rejection suspends local input and calls failure hook', () {
    final failures = <int>[];
    final commands = <Map<String, dynamic>>[];
    final engine = ControllerCanvasEngine(library, (command) {
      commands.add(command);
      return command['kind'] == 'button' ? 4 : 0;
    }, onFailure: failures.add);
    addTearDown(engine.dispose);
    configure(engine);
    final state = engine.state;
    final center = ControllerInputPoint(1, state.imageX + state.imageWidth / 2,
        state.imageY + state.imageHeight / 2);
    touch(engine, ControllerTouchAction.down, [center], center, 0);
    expect(touch(engine, ControllerTouchAction.up, [], center, 20), 4);
    expect(failures, [4]);
    expect(engine.state.inputEnabled, false);
    expect(
        commands.where((command) => command['kind'] == 'button'), hasLength(1));
  });

  testWidgets(
      'canvas view applies MediaQuery insets and reset overlay consumes tap',
      (tester) async {
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final commands = <Map<String, dynamic>>[];
    final engine = canvas(commands);
    addTearDown(engine.dispose);
    Widget view() => MediaQuery(
          data: const MediaQueryData(
              size: Size(400, 800),
              padding: EdgeInsets.only(top: 20, bottom: 30)),
          child: MaterialApp(
            home: SizedBox(
              width: 400,
              height: 800,
              child: ControllerCanvasView(
                engine: engine,
                remoteSize: const Size(1920, 1080),
                inputEnabled: true,
                topOverlayHeight: 40,
                bottomOverlayHeight: 20,
                remoteFrameBuilder: (context, size) =>
                    const ColoredBox(color: Colors.red),
              ),
            ),
          ),
        );
    await tester.pumpWidget(view());
    await tester.pump();
    expect(engine.state.safeY, 84);
    expect(engine.state.safeHeight, 642);

    const a = ControllerInputPoint(1, 150, 300);
    var b = const ControllerInputPoint(2, 250, 300);
    touch(engine, ControllerTouchAction.down, [a], a, 0);
    touch(engine, ControllerTouchAction.down, [a, b], b, 10);
    b = const ControllerInputPoint(2, 350, 300);
    touch(engine, ControllerTouchAction.move, [a, b], b, 30);
    await tester.pump();
    expect(find.byTooltip('还原画面'), findsOneWidget);
    commands.clear();
    await tester.tap(find.byTooltip('还原画面'));
    await tester.pump();
    expect(engine.state.zoom, 1);
    expect(find.byTooltip('还原画面'), findsNothing);
    expect(commands, isEmpty);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('canvas long-press tick is single shot and removed on unmount',
      (tester) async {
    final commands = <Map<String, dynamic>>[];
    final engine = canvas(commands);
    addTearDown(engine.dispose);
    await tester.pumpWidget(MaterialApp(
      home: ControllerCanvasView(
        engine: engine,
        remoteSize: const Size(1920, 1080),
        inputEnabled: true,
        remoteFrameBuilder: (context, size) =>
            const ColoredBox(color: Colors.red),
      ),
    ));
    await tester.pump();
    final gesture = await tester.startGesture(const Offset(200, 200),
        kind: PointerDeviceKind.touch);
    await tester.pump();
    expect(engine.state.nextTickMs, greaterThan(0));
    await tester.pump(const Duration(milliseconds: 1000));
    expect(engine.state.dragReady, true);
    expect(find.text('拖动准备'), findsOneWidget);
    await gesture.up();
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
    expect(tester.takeException(), isNull);
  });
}
