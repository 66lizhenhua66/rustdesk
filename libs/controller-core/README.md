# Controller core

This independent Rust workspace builds configuration validation, read-only
network preflight, strict authentication, persistent DEMO sessions, and the formal
VP8 video and separately authorized input protocol. It directly compiles the upstream
`../hbb_common/src/bytes_codec.rs` and generates Rust from the shared
`../base/protos/message.proto` and the rendezvous definitions at build time. Those protocol sources are not
copied into this crate.

The C ABI is declared in `include/controller.h`, `include/session.h`, and `include/input.h`.
Preflight never authenticates. Strict login requires an explicitly pinned peer ID
and Ed25519 public key, validates the signed ephemeral key, requires key exchange
v1, and accepts only encrypted challenges/results. A successful login reports
`authenticated:true` and immediately closes. This original single-login API keeps
`authorized:false`.

The platform-neutral input engine in `src/interaction.rs` owns touch gestures,
pointer coordinates, wheel accumulation, physical key state and UTF-8 text
chunking. Harmony's NAPI adapter and `flutter/lib/controller_input/` call the
same `controller_input_*_v1` ABI. Each engine has an exclusive opaque handle;
its synchronous sink must route generated commands through the strict session
API. The engine never grants access. A failed sink aborts remaining commands;
the adapter must disable session input or disconnect. See
[cross-platform input](../../docs/project/specs/004-cross-platform-input.md).

Default static builds remain unchanged. Build a Windows FFI test library with
`cargo rustc --manifest-path libs/controller-core/Cargo.toml --lib --crate-type cdylib --locked`
from the repository root. Android/iOS packaging and actual device validation
remain separate work; a Windows DLL test is not a mobile build result.

`controller_connection_create` selects a persistent protocol with `expectedPeer`:

| Mode | Behavior |
| --- | --- |
| `demo` (default) | Requires the DEMO capability marker. Existing 800x450 pointer and 512-byte UTF-8 text APIs affect only `apps/windows-demo-host`, after its encrypted Keyboard grant. |
| `secure_host` | Formal non-media connection, with pinned identity, v1 encryption and local CM approval; input remains disabled. |
| `secure_video` | Formal read-only VP8; requires `controller_session_run_video`, rejects input permission escalation. |
| `secure_control` | Explicit input-v2 negotiation plus VP8; also uses `controller_session_run_video`. Connection approval admits the session, while input remains disabled until the controller requests it. |

Mode and capability mismatches fail; there is no automatic downgrade. New Harmony
Desk uses `secure_control`; the diagnostic Index retains its old read-only and
DEMO paths. Video bytes go directly to the native video callback, never through
ArkTS/JSON. Media decoding, OS injection and file transfer are outside this crate.

## ID, relay and trusted read-only access

Requests accept `mode` (default `direct`), `server`, `serverKey` and `relayServer`.
ID/relay require explicit endpoints plus pinned server and peer signing keys.
TCP coordination can fall back to the configured relay only on TCP failure;
identity failures never downgrade. A `transport_selected` event reports actual
`connectionPath=direct|id_direct|relay` without implying authentication or approval.
Only TCP coordination is implemented. Hostnames resolve with bounded workers;
UDP/WebRTC and public-server defaults are not provided.

`controller_connection_create_access_v1` takes the public request and a separate
secret JSON, selects `secure_video` only, and implements `pair`/`unattended` with
a fresh challenge proof. Ordinary APIs retain onsite approval. The dedicated
identity creation and secret-string-free APIs are declared in `include/session.h`.
`access_paired` contains one grant for platform secure storage; it must never be
logged or copied into a profile. `connected.accessMode` confirms the requested
mode. The host requires explicit local enrollment, validates credentials on every
connection, and checks revocation/expiry/policy each second. No 2FA exemption is
implemented. [Protocol and storage contract](../../docs/project/specs/003-trusted-controller-access.md).

The historical profile ABI `readyForSession=false` field remains for legacy
diagnostics; use `authenticationAvailable`, complete configuration and actual
session events for the implemented strict connection modes. This does not claim
full upstream Client/Session compatibility.

2026-10-05: 107 core/policy/loopback tests pass. Actual self-hosted hbbs/hbbr,
public networks, Asset Store device behavior and unattended UI acceptance remain
separate checks; see the [validation record](../../docs/project/research/2026-10-05-id-relay-unattended-validation.md).

## System input capability v2

`controller_session_send_input_v1(session, json)` accepts `move` (0..65535 primary
screen coordinates), `move_relative` (signed normalized deltas -65535..65535),
`button`, `wheel` (-10..10 steps per axis), allowlisted `key` (including Space),
`text` (1..512 UTF-8 bytes) and `release_all`. Unknown fields and unsupported input
are rejected. Existing DEMO send APIs keep their original scope.

Windows requires local `ORD_SECURE_INPUT=1` in addition to its video gate. This is an
implementation capability gate, not a second user approval. Windows approves or
rejects each connection once; after admission the controller requests enable/disable
with `OrdInputRequest` (scope `windows_primary`, `request_id`). Only one connection
owns input at a time. Each enable uses a fresh random 16-byte token, held exclusively
in Rust and attached at send time. Neither the C ABI command JSON nor ArkUI receives the
token. The authenticated `input_state` event reports `inputSupported`, `authorized`
and the correlated `request_id`; stale responses cannot reverse a newer choice. The
controller defaults to read-only and must wait for the real state before sending input.

The C ABI `controller_session_set_input_enabled_v1(session, uint8_t enabled)` requests
the controller's choice; `_v1` versions the C ABI and does not name the negotiated
input capability. `controller_session_send_input_v1` remains the event API for
move/move_relative/button/wheel/key/text/release_all, but queue acceptance never
implicitly enables input. Selecting read-only, closing controls, hiding a session or
disconnecting requests disable and stops local input immediately. Grant tokens remain
inside Rust.

The bounded queue validates authorization before enqueueing and sending. Revocation,
cancellation and disconnect clear pending commands; release commands preempt a full
queue. Return codes are 0 queued, 1 disconnected/wrong mode, 2 not authorized,
3 invalid command and 4 queue full. Queue acceptance is not proof of OS execution.

Granted control uses internal bidirectional heartbeats: Windows revokes and releases
held inputs after 3 seconds without valid input/heartbeat; the core closes a granted
connection after 5 seconds without inbound server messages. Windows checks the
current grant, interactive desktop and primary display at the actual injection point.
Failed cleanup explicitly fails closed and blocks further grants until the host
process restarts. These checks do not open UAC, elevation or secure desktops.

2026-10-04 validation: 84 core tests passed. Combined with 9 legacy DEMO, 15 Windows,
4 Flutter widget and 10 Harmony model tests, the slice has 122 automated passes.
Windows Debug and both OHOS ABI/HAP builds passed; the phone emulator has the new
HAP. The v2 runtime trial is awaiting a new local connection approval, so actual
Windows input, Chinese text entry, enable/disable switching and cleanup have **not**
been verified. See the [controller capability validation record](../../docs/project/research/2026-10-04-controller-capability-validation.md).

Passwords are passed separately for one call and held in zeroizing Rust buffers;
they are not part of saved profiles. An empty password waits for the peer's
explicit login response. A second-factor request remains blocked; this version
does not continue the 2FA exchange. Cancellation shuts down a connected socket.

## Upstream security code

`build.rs` checks normalized SHA-256 hashes of `src/common.rs` and
`libs/hbb_common/src/tcp.rs`, then emits their identity helpers and Encrypt
implementation into OUT_DIR. No upstream runtime source is changed or copied
into a separately maintained implementation. The extracted upstream crypto
tests receive required sodium initialization; runtime algorithm bodies are
unchanged. A source change fails the build until the extraction and fixed wire
vectors are reviewed. This is a transitional integration seam, not a stable
upstream SDK.

- tcp.rs: `f354dc407bf30ff13f8289c4efe1e3f031333748e0da67ec401830f74285debc`
- common.rs: `d9bc63c5c504213c9413a7fe9fa07405273f44ecdb2f374aa52f07d818c6a21d`

OHOS libsodium build inputs, hashes and licenses are documented in
[native/README.md](native/README.md). The app build script builds the pinned
library for both OHOS ABIs and sets target-specific SODIUM_LIB_DIR only while
building; Windows host tests use the dependency's bundled MSVC library.

The original RustDesk direct-IP listener does not emit this signed handshake.
It is deliberately rejected. The Windows DEMO and the fork's explicit formal entry
provide the required signed handshake; this does not establish compatibility with an
unmodified RustDesk deployment. See the [formal entry guide](../../docs/project/OFFICIAL-SECURE-ENTRY.md)
and [DEMO guide](../../docs/project/DEMO.md).
