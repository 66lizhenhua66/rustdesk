import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'controller_canvas_engine.dart';
import 'flutter_canvas_input_adapter.dart';
import 'flutter_input_adapter.dart';

typedef ControllerRemoteFrameBuilder = Widget Function(
    BuildContext context, Size remoteSize);

/// A full viewport canvas around a caller-owned decoded remote frame.
/// The owner supplies strict session grant state through [inputEnabled] and
/// revokes remote input before disposing the engine.
class ControllerCanvasView extends StatefulWidget {
  const ControllerCanvasView({
    super.key,
    required this.engine,
    required this.remoteSize,
    required this.inputEnabled,
    required this.remoteFrameBuilder,
    this.visible = true,
    this.topOverlayHeight = 0,
    this.bottomOverlayHeight = 0,
    this.padding = 24,
  });

  final ControllerCanvasEngine engine;
  final Size remoteSize;
  final bool inputEnabled;
  final bool visible;
  final double topOverlayHeight;
  final double bottomOverlayHeight;
  final double padding;
  final ControllerRemoteFrameBuilder remoteFrameBuilder;

  @override
  State<ControllerCanvasView> createState() => _ControllerCanvasViewState();
}

class _ControllerCanvasViewState extends State<ControllerCanvasView>
    with WidgetsBindingObserver {
  late FlutterCanvasInputAdapter _adapter;
  final FocusNode _focusNode = FocusNode();
  Timer? _tickTimer;
  int _lastPointerMs = 0;
  int _scheduledTick = 0;
  int _firedTick = 0;
  String? _appliedConfig;
  String? _pendingConfig;

  @override
  void initState() {
    super.initState();
    _adapter = FlutterCanvasInputAdapter(widget.engine);
    widget.engine.addListener(_stateChanged);
    WidgetsBinding.instance.addObserver(this);
    _focusNode.addListener(_focusChanged);
  }

  @override
  void didUpdateWidget(covariant ControllerCanvasView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.engine != widget.engine) {
      _stopOldEngine(oldWidget);
      oldWidget.engine.removeListener(_stateChanged);
      _adapter = FlutterCanvasInputAdapter(widget.engine);
      widget.engine.addListener(_stateChanged);
      _appliedConfig = null;
      _pendingConfig = null;
    } else if ((oldWidget.visible && !widget.visible) ||
        (oldWidget.inputEnabled && !widget.inputEnabled)) {
      _releaseOrSuspend(oldWidget.inputEnabled && widget.inputEnabled);
    }
    if (!oldWidget.visible && widget.visible) {
      _pendingConfig = null;
      _appliedConfig = null;
    }
    if (!widget.visible) _cancelTick();
  }

  void _stopOldEngine(ControllerCanvasView oldWidget) {
    _cancelTick();
    _adapter.clearLocal();
    if (oldWidget.inputEnabled && oldWidget.engine.state.inputEnabled) {
      oldWidget.engine.release();
    } else {
      oldWidget.engine.suspend();
    }
  }

  void _releaseOrSuspend(bool canSend) {
    _cancelTick();
    _adapter.clearLocal();
    if (canSend && widget.engine.state.inputEnabled) {
      widget.engine.release();
    } else {
      widget.engine.suspend();
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) {
      _releaseOrSuspend(widget.inputEnabled);
    }
  }

  void _focusChanged() {
    if (!_focusNode.hasFocus) _releaseOrSuspend(widget.inputEnabled);
  }

  void _stateChanged() {
    if (!mounted) return;
    _scheduleTick();
    setState(() {});
  }

  void _scheduleTick() {
    final next = widget.visible ? widget.engine.state.nextTickMs : 0;
    if (next == 0) _firedTick = 0;
    if (next == _scheduledTick) return;
    _tickTimer?.cancel();
    _tickTimer = null;
    _scheduledTick = next;
    if (next <= 0 || next == _firedTick) return;
    final delay = math.max(0, next - _lastPointerMs);
    _tickTimer = Timer(Duration(milliseconds: delay), () {
      _tickTimer = null;
      _scheduledTick = 0;
      _firedTick = next;
      if (mounted && widget.visible) widget.engine.tick(next);
    });
  }

  void _cancelTick() {
    _tickTimer?.cancel();
    _tickTimer = null;
    _scheduledTick = 0;
  }

  void _pointer(PointerEvent event) {
    _lastPointerMs = event.timeStamp.inMilliseconds;
    if (event is PointerDownEvent) _focusNode.requestFocus();
    _adapter.pointer(event);
  }

  void _configure(Size viewport, EdgeInsets mediaPadding) {
    final insets = CanvasInsets(
      left: mediaPadding.left,
      top: mediaPadding.top + widget.topOverlayHeight,
      right: mediaPadding.right,
      bottom: mediaPadding.bottom + widget.bottomOverlayHeight,
    );
    final key = jsonEncode([
      viewport.width,
      viewport.height,
      widget.remoteSize.width,
      widget.remoteSize.height,
      insets.toJson(),
      widget.padding,
      widget.inputEnabled,
    ]);
    if (key == _appliedConfig || key == _pendingConfig) return;
    _pendingConfig = key;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !widget.visible || _pendingConfig != key) return;
      _pendingConfig = null;
      _adapter.clearLocal();
      widget.engine.configure(
        viewportWidth: viewport.width,
        viewportHeight: viewport.height,
        remoteWidth: widget.remoteSize.width,
        remoteHeight: widget.remoteSize.height,
        insets: insets,
        padding: widget.padding,
        enabled: widget.inputEnabled,
      );
      _appliedConfig = key;
    });
  }

  KeyEventResult _key(KeyEvent event) {
    if (FlutterInputAdapter.keyCode(event.physicalKey, event.logicalKey) ==
        null) {
      return KeyEventResult.ignored;
    }
    _adapter.key(event);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox.shrink();
    final mediaPadding = MediaQuery.maybePaddingOf(context) ?? EdgeInsets.zero;
    return LayoutBuilder(builder: (context, constraints) {
      final viewport = constraints.biggest;
      _configure(viewport, mediaPadding);
      final state = widget.engine.state;
      final rightInset =
          math.max(0.0, viewport.width - state.safeX - state.safeWidth);
      return Focus(
        focusNode: _focusNode,
        onKeyEvent: (_, event) => _key(event),
        child: Material(
          type: MaterialType.transparency,
          child: Stack(
            clipBehavior: Clip.hardEdge,
            children: [
              Listener(
                behavior: HitTestBehavior.opaque,
                onPointerDown: _pointer,
                onPointerMove: _pointer,
                onPointerUp: _pointer,
                onPointerCancel: _pointer,
                onPointerHover: _pointer,
                onPointerSignal: _adapter.pointerSignal,
                child: SizedBox.expand(
                  child: ColoredBox(
                    color: Colors.black,
                    child: ClipRect(
                      clipper: _CanvasSafeClipper(Rect.fromLTWH(state.safeX,
                          state.safeY, state.safeWidth, state.safeHeight)),
                      child: Stack(clipBehavior: Clip.hardEdge, children: [
                        if (state.imageWidth > 0 && state.imageHeight > 0)
                          Positioned(
                            left: state.imageX,
                            top: state.imageY,
                            width: state.imageWidth,
                            height: state.imageHeight,
                            child: IgnorePointer(
                              child: FittedBox(
                                fit: BoxFit.fill,
                                child: SizedBox(
                                  width: widget.remoteSize.width,
                                  height: widget.remoteSize.height,
                                  child: widget.remoteFrameBuilder(
                                      context, widget.remoteSize),
                                ),
                              ),
                            ),
                          ),
                      ]),
                    ),
                  ),
                ),
              ),
              if (state.dragReady && state.safeWidth > 0)
                Positioned(
                  left: state.safeX + 8,
                  top: state.safeY + 8,
                  child: const IgnorePointer(
                    child: Chip(label: Text('拖动准备')),
                  ),
                ),
              if (state.resetVisible && state.safeWidth > 0)
                Positioned(
                  top: state.safeY + 8,
                  right: rightInset + 8,
                  child: Row(mainAxisSize: MainAxisSize.min, children: [
                    Text('${state.zoom.toStringAsFixed(1)}x',
                        style: const TextStyle(color: Colors.white)),
                    IconButton.filled(
                      tooltip: '还原画面',
                      icon: const Icon(Icons.fit_screen),
                      onPressed: () {
                        _cancelTick();
                        _adapter.clearLocal();
                        widget.engine.resetView();
                      },
                    ),
                  ]),
                ),
            ],
          ),
        ),
      );
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _cancelTick();
    _focusNode.removeListener(_focusChanged);
    _focusNode.dispose();
    _adapter.clearLocal();
    widget.engine.removeListener(_stateChanged);
    if (widget.inputEnabled && widget.engine.state.inputEnabled) {
      widget.engine.release();
    } else {
      widget.engine.suspend();
    }
    super.dispose();
  }
}

class _CanvasSafeClipper extends CustomClipper<Rect> {
  const _CanvasSafeClipper(this.rect);

  final Rect rect;

  @override
  Rect getClip(Size size) => rect;

  @override
  bool shouldReclip(covariant _CanvasSafeClipper oldClipper) =>
      oldClipper.rect != rect;
}
