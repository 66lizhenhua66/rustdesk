# Controller core

This independent Rust workspace builds the non-media controller validation and
read-only network preflight static library. It directly compiles the upstream
`../hbb_common/src/bytes_codec.rs` and generates Rust from the upstream
`../base/protos/message.proto` at build time. Those protocol sources are not
copied into this crate.

The C ABI is declared in `include/controller.h`. It does not implement a
RustDesk authenticated session, input, file transfer, or video. Every probe
event reports `verified:false` and `authorized:false`.
