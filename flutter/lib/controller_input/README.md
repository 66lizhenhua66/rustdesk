# Flutter controller input adapter

`ControllerInputEngine` calls the versioned C ABI in
`libs/controller-core/include/input.h`. `FlutterInputAdapter` converts Flutter
pointer, scroll and key events into that ABI. Rust owns gesture recognition,
coordinate conversion, key state and Unicode text chunking. No FRB generation
or additional Dart package is needed.

Construct the engine with a `DynamicLibrary`, a synchronous command sink and a
required `onFailure(status)` callback. The real sink must forward commands to
`controller_session_send_input_v1` for the current strict session; successful
queueing does not prove remote execution or grant authorization. `onFailure`
must immediately call `controller_session_set_input_enabled_v1(session, 0)`;
if that fails, cancel the session. The engine clears only local gesture state
when a sink fails. Caught sink exceptions are available as `sinkError` and also
trigger `onFailure`; native callback JSON is copied before the callback returns.
Neither callback may reenter the same input engine or return a Future.

Use the remote image's local viewport for the adapter. Connect its `pointer`,
`pointerSignal`, `key`, `focusChanged` and `commitText` methods to the feature's
Flutter event handlers. Pointer scroll deltas use Flutter's direction; the
adapter reverses the vertical delta for the shared remote wheel convention. Wire IME
commits to `commitText` and avoid also sending those committed characters as
hardware keys. Input enablement remains the owning session's responsibility.

On pointer cancellation, focus loss, hiding the view or viewport changes, call
`release()` (viewport changes can use `updateViewport`). `reset()` emits no remote
release. Disable remote input or disconnect before `reset()` or `dispose()`;
dispose releases both the native handle and its isolate-local callback. A
failed release follows the same mandatory `onFailure` path.

The adapter source supports Flutter Windows, Android and iOS control clients.
It is not wired into their production UI. Android/iOS native library builds,
packaging, symbol retention, signing and device validation are still pending;
Windows controller library bundling is also pending. Windows host `SendInput`
is a separate controlled-device role.

Run the native smoke tests from `rustdesk/` after building controller-core:

```powershell
cargo rustc --manifest-path libs/controller-core/Cargo.toml --lib --crate-type cdylib --locked
$env:CONTROLLER_INPUT_LIBRARY = (Resolve-Path 'libs/controller-core/target/debug/remote_controller_core.dll').Path
Push-Location flutter
& '..\..\.tools\flutter-3.24.5\flutter\bin\flutter.bat' test --no-pub test/controller_input_ffi_test.dart
Pop-Location
```

Missing `CONTROLLER_INPUT_LIBRARY`, missing DLLs or missing ABI symbols fail
these tests; they are never silently skipped. Passing on Windows verifies Dart
bindings and the shared engine, not Android/iOS packaging or remote injection.
