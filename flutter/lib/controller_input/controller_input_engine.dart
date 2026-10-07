import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

typedef _Sink = Int32 Function(Pointer<Void>, Pointer<Utf8>);
typedef _EventNative = Int32 Function(Pointer<Void>, Pointer<Utf8>,
    Pointer<NativeFunction<_Sink>>, Pointer<Void>);
typedef _EventDart = int Function(Pointer<Void>, Pointer<Utf8>,
    Pointer<NativeFunction<_Sink>>, Pointer<Void>);

enum ControllerTouchAction { down, up, move, cancel }

enum ControllerMouseAction { press, release, move, cancel }

enum ControllerMouseButton { left, right, middle }

class ControllerInputPoint {
  const ControllerInputPoint(this.id, this.x, this.y);

  final int id;
  final double x;
  final double y;

  Map<String, Object> _toJson() => {'id': id, 'x': x, 'y': y};
}

/// Synchronous, isolate-local bindings to controller-core's input C ABI v1.
/// The sink must call the strict session API. On failure, [onFailure] must
/// disable session input (or disconnect if disabling fails).
class ControllerInputEngine {
  ControllerInputEngine(DynamicLibrary library, this._sink,
      {required this.onFailure}) {
    final create = library.lookupFunction<Pointer<Void> Function(),
        Pointer<Void> Function()>('controller_input_new_v1');
    _event = library
        .lookupFunction<_EventNative, _EventDart>('controller_input_event_v1');
    _reset = library.lookupFunction<Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('controller_input_reset_v1');
    _free = library.lookupFunction<Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('controller_input_free_v1');
    _handle = create();
    if (_handle == nullptr) throw StateError('Cannot create input engine');
    try {
      _callback = NativeCallable<_Sink>.isolateLocal(
          (Pointer<Void> context, Pointer<Utf8> command) {
        try {
          // Native JSON expires when this callback returns.
          return _sink(
              jsonDecode(command.toDartString()) as Map<String, dynamic>);
        } catch (error) {
          _sinkError = error;
          return 3;
        }
      }, exceptionalReturn: 3);
    } catch (_) {
      _free(_handle);
      rethrow;
    }
  }

  final int Function(Map<String, dynamic>) _sink;
  final void Function(int status) onFailure;
  late final _EventDart _event;
  late final void Function(Pointer<Void>) _reset;
  late final void Function(Pointer<Void>) _free;
  late final NativeCallable<_Sink> _callback;
  late Pointer<Void> _handle;
  bool _dispatching = false;
  Object? _sinkError;

  Object? get sinkError => _sinkError;

  int touch({
    required ControllerTouchAction action,
    required List<ControllerInputPoint> points,
    required List<ControllerInputPoint> changedPoints,
    required int time,
    required double width,
    required double height,
  }) =>
      _dispatch({
        'kind': 'touch',
        'action': action.name,
        'points': points.map((point) => point._toJson()).toList(),
        'changedPoints': changedPoints.map((point) => point._toJson()).toList(),
        'time': time,
        'width': width,
        'height': height,
      });

  int mouse({
    required ControllerMouseAction action,
    ControllerMouseButton? button,
    required double x,
    required double y,
    required double width,
    required double height,
  }) =>
      _dispatch({
        'kind': 'mouse',
        'action': action.name,
        'button': button?.name ?? '',
        'x': x,
        'y': y,
        'width': width,
        'height': height,
      });

  int wheel({
    required double x,
    required double y,
    required double dx,
    required double dy,
    required bool discrete,
    required double width,
    required double height,
  }) =>
      _dispatch({
        'kind': 'wheel',
        'action': 'update',
        'x': x,
        'y': y,
        'dx': dx,
        'dy': dy,
        'discrete': discrete,
        'width': width,
        'height': height,
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

  int _dispatch(Map<String, Object> event) {
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
        _reset(_handle);
        onFailure(status);
      }
      return status;
    } finally {
      if (json != nullptr) malloc.free(json);
      _dispatching = false;
    }
  }

  /// Clear local state only, after remote input was disabled or disconnected.
  void reset() {
    _checkIdle();
    _reset(_handle);
  }

  /// The caller must first disable remote input or disconnect the session.
  void dispose() {
    if (_handle == nullptr) return;
    _checkIdle();
    _free(_handle);
    _handle = nullptr;
    _callback.close();
  }

  void _checkIdle() {
    if (_handle == nullptr) throw StateError('Input engine is disposed');
    if (_dispatching) throw StateError('Input engine cannot be reentered');
  }
}
