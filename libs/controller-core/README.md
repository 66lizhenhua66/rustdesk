# Controller core

This independent Rust workspace builds configuration validation, read-only
network preflight, strict single-login verification, and an isolated persistent DEMO session. It directly compiles the upstream
`../hbb_common/src/bytes_codec.rs` and generates Rust from the upstream
`../base/protos/message.proto` and the rendezvous definitions at build time. Those protocol sources are not
copied into this crate.

The C ABI is declared in `include/controller.h` and `include/session.h`.
Preflight never authenticates. Strict login requires an explicitly pinned peer ID
and Ed25519 public key, validates the signed ephemeral key, requires key exchange
v1, and accepts only encrypted challenges/results. A successful login reports
`authenticated:true` and immediately closes. This original single-login API keeps
`authorized:false`.

The separate `controller_connection_create` API requires the explicit DEMO
capability marker and remains connected. Input starts disabled and is enabled
only by an encrypted Keyboard permission update. It accepts a bounded queue of
800x450 pointer positions and UTF-8 text up to 512 bytes, exclusively for
`apps/windows-demo-host`. Cancellation and revocation clear pending input.
Queue acceptance does not mean execution; encrypted `demo_status` events report
the host's current position and text length. No full-desktop input, file transfer,
media decoder, ID discovery, or relay channel is implemented.

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
It is deliberately rejected. The Windows DEMO host provides the strict signed
entry point and local approval; this does not establish compatibility with an
unmodified Windows RustDesk deployment. See the [DEMO guide](../../docs/project/DEMO.md).
