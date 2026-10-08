import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';

import 'controller_input_engine.dart';

typedef _Sink = Int32 Function(Pointer<Void>, Pointer<Utf8>);
typedef _EventNative = Int32 Function(Pointer<Void>, Pointer<Utf8>,
    Pointer<NativeFunction<_Sink>>, Pointer<Void>);
typedef _EventDart = int Function(Pointer<Void>, Pointer<Utf8>,
    Pointer<NativeFunction<_Sink>>, Pointer<Void>);

enum CanvasTouchMode { direct, pointer }

class CanvasInsets {
  const CanvasInsets(
      {this.left = 0, this.top = 0, this.right = 0, this.bottom = 0});

  final double left;
  final double top;
  final double right;
  final double bottom;

  Map<String, double> toJson() => {
        'left': left,
        'top': top,
        'right': right,
        'bottom': bottom,
      };
}

class ControllerCanvasState {
  const ControllerCanvasState._(
      this.zoom,
      this.imageX,
      this.imageY,
      this.imageWidth,
      this.imageHeight,
      this.safeX,
      this.safeY,
      this.safeWidth,
      this.safeHeight,
      this.dragReady,
      this.resetVisible,
      this.gesture,
      this.nextTickMs,
      this.inputEnabled,
      this.touchMode);

  factory ControllerCanvasState.fromJson(Map<String, dynamic> json) =>
      ControllerCanvasState._(
        (json['zoom'] as num).toDouble(),
        (json['imageX'] as num).toDouble(),
        (json['imageY'] as num).toDouble(),
        (json['imageWidth'] as num).toDouble(),
        (json['imageHeight'] as num).toDouble(),
        (json['safeX'] as num).toDouble(),
        (json['safeY'] as num).toDouble(),
        (json['safeWidth'] as num).toDouble(),
        (json['safeHeight'] as num).toDouble(),
        json['dragReady'] as bool,
        json['resetVisible'] as bool,
        json['gesture'] as String,
        json['nextTickMs'] as int,
        json['inputEnabled'] as bool,
        CanvasTouchMode.values.byName(json['touchMode'] as String),
      );

  final double zoom;
  final double imageX;
  final double imageY;
  final double imageWidth;
  final double imageHeight;
  final double safeX;
  final double safeY;
  final double safeWidth;
  final double safeHeight;
  final bool dragReady;
  final bool resetVisible;
  final String gesture;
  final int nextTickMs;
  final bool inputEnabled;
  final CanvasTouchMode touchMode;
}

/// Isolate-local canvas ABI. The sink must use the strict session input API.
/// [onFailure] must disable session input or disconnect it.
class ControllerCanvasEngine extends ChangeNotifier {
  ControllerCanvasEngine(DynamicLibrary library, this._sink,
      {required this.onFailure}) {
    final create = library.lookupFunction<Pointer<Void> Function(),
        Pointer<Void> Function()>('controller_canvas_new_v1');
    _event = library
        .lookupFunction<_EventNative, _EventDart>('controller_canvas_event_v1');
    _nativeState = library.lookupFunction<Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('controller_canvas_state_v1');
    _freeString = library.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('controller_free_string');
    _suspend = library.lookupFunction<Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('controller_canvas_suspend_v1');
    _free = library.lookupFunction<Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('controller_canvas_free_v1');
    _handle = create();
    if (_handle == nullptr) throw StateError('Cannot create canvas engine');
    var callbackReady = false;
    try {
      _callback = NativeCallable<_Sink>.isolateLocal(
          (Pointer<Void> context, Pointer<Utf8> command) {
        try {
          return _sink(
              jsonDecode(command.toDartString()) as Map<String, dynamic>);
        } catch (error) {
          _sinkError = error;
          return 3;
        }
      }, exceptionalReturn: 3);
      callbackReady = true;
      _state = _readState();
    } catch (_) {
      if (callbackReady) _callback.close();
      _free(_handle);
      rethrow;
    }
  }

  final int Function(Map<String, dynamic>) _sink;
  final void Function(int status) onFailure;
  late final _EventDart _event;
  late final Pointer<Utf8> Function(Pointer<Void>) _nativeState;
  late final void Function(Pointer<Utf8>) _freeString;
  late final void Function(Pointer<Void>) _suspend;
  late final void Function(Pointer<Void>) _free;
  late final NativeCallable<_Sink> _callback;
  late Pointer<Void> _handle;
  late ControllerCanvasState _state;
  bool _dispatching = false;
  Object? _sinkError;

  ControllerCanvasState get state => _state;
  Object? get sinkError => _sinkError;

  int configure({
    required double viewportWidth,
    required double viewportHeight,
    required double remoteWidth,
    required double remoteHeight,
    CanvasInsets insets = const CanvasInsets(),
    double padding = 24,
    required bool enabled,
  }) =>
      _dispatch({
        'kind': 'configure',
        'viewportWidth': viewportWidth,
        'viewportHeight': viewportHeight,
        'remoteWidth': remoteWidth,
        'remoteHeight': remoteHeight,
        'insets': insets.toJson(),
        'padding': padding,
        'enabled': enabled,
      });

  int touch({
    required ControllerTouchAction action,
    required List<ControllerInputPoint> points,
    required List<ControllerInputPoint> changedPoints,
    required int time,
  }) =>
      _dispatch({
        'kind': 'touch',
        'action': action.name,
        'points': points.map(_pointJson).toList(),
        'changedPoints': changedPoints.map(_pointJson).toList(),
        'time': time,
      });

  static Map<String, Object> _pointJson(ControllerInputPoint point) =>
      {'id': point.id, 'x': point.x, 'y': point.y};

  int mouse({
    required ControllerMouseAction action,
    ControllerMouseButton? button,
    required double x,
    required double y,
  }) =>
      _dispatch({
        'kind': 'mouse',
        'action': action.name,
        'button': button?.name ?? '',
        'x': x,
        'y': y,
      });

  int wheel({
    required double x,
    required double y,
    required double dx,
    required double dy,
    required bool discrete,
  }) =>
      _dispatch({
        'kind': 'wheel',
        'action': 'update',
        'x': x,
        'y': y,
        'dx': dx,
        'dy': dy,
        'discrete': discrete,
      });

  int key(
          {required bool down,
          required int physicalCode,
          required String code}) =>
      _dispatch({
        'kind': 'key',
        'action': down ? 'down' : 'up',
        'physicalCode': physicalCode,
        'code': code,
      });

  int commitText(String text) => _dispatch({'kind': 'text', 'text': text});
  int release() => _dispatch({'kind': 'release'});
  int resetView() => _dispatch({'kind': 'reset_view'});
  int setTouchMode(CanvasTouchMode mode) =>
      _dispatch({'kind': 'touch_mode', 'mode': mode.name});
  int tick(int time) => _dispatch({'kind': 'tick', 'time': time});

  int _dispatch(Map<String, Object?> event) {
    _checkIdle();
    _dispatching = true;
    _sinkError = null;
    Pointer<Utf8> json = nullptr;
    try {
      int status;
      try {
        json = jsonEncode(event).toNativeUtf8();
        status = _event(_handle, json, _callback.nativeFunction, nullptr);
      } catch (error) {
        _sinkError = error;
        status = 3;
      }
      if (status != 0) {
        _suspend(_handle);
        onFailure(status);
      }
      _state = _readState();
      notifyListeners();
      return status;
    } finally {
      if (json != nullptr) malloc.free(json);
      _dispatching = false;
    }
  }

  ControllerCanvasState _readState() {
    final value = _nativeState(_handle);
    if (value == nullptr) throw StateError('Cannot read canvas state');
    try {
      return ControllerCanvasState.fromJson(
          jsonDecode(value.toDartString()) as Map<String, dynamic>);
    } finally {
      _freeString(value);
    }
  }

  /// Call after remote input has been revoked or the session disconnected.
  void suspend() {
    _checkIdle();
    _suspend(_handle);
    _state = _readState();
    notifyListeners();
  }

  /// The owner must revoke remote input or disconnect before disposal.
  @override
  void dispose() {
    if (_handle == nullptr) return;
    _checkIdle();
    _suspend(_handle);
    _free(_handle);
    _handle = nullptr;
    _callback.close();
    super.dispose();
  }

  void _checkIdle() {
    if (_handle == nullptr) throw StateError('Canvas engine is disposed');
    if (_dispatching) throw StateError('Canvas engine cannot be reentered');
  }
}
