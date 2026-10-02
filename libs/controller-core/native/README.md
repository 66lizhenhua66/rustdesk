# Native libraries for OHOS

## libvpx for OHOS

`build-libvpx-ohos.ps1` builds decoder-only libvpx 1.15.2 for `arm64-v8a` and
`x86_64` with the DevEco Native SDK. It verifies the archive SHA512, refreshes
its own `artifacts/libvpx-ohos/source/` extraction, and builds only that source.
It downloads the pinned archive when the local copy is absent. The output is
`artifacts/libvpx-ohos/<ABI>/lib/libvpx.a` with matching headers in `include/`.
The static archives and intermediate build directories are ignored by Git.
Pass `-MsysBin` when the vcpkg MSYS2 tools are installed elsewhere.

Source: [webmproject/libvpx v1.15.2](https://github.com/webmproject/libvpx/releases/tag/v1.15.2),
archive `webmproject-libvpx-v1.15.2.tar.gz`, SHA512
`824fe8719e4115ec359ae0642f5e1cea051d458f09eb8c24d60858cf082f66e411215e23228173ab154044bafbdfbb2d93b589bb726f55b233939b91f928aae0`.
License: BSD 3-Clause, reproduced in the source archive's `LICENSE` file.
The build disables assembly and the encoder for a portable VP8 decoder.

## libsodium for OHOS

`build-libsodium-ohos.ps1` builds the libsodium 1.0.18 source bundled with
`libsodium-sys` 0.2.7 for `arm64-v8a` and `x86_64` using the DevEco Native SDK.
Run from PowerShell with an explicit `-SdkNative` path on other machines.
The output libraries are `artifacts/build-arm64-v8a/libsodium.a` and
`artifacts/build-x86_64/libsodium.a`; `artifacts/` is ignored by Git.

The input archives are pinned and checked before extraction:

| Input | Revision | SHA256 | License |
| --- | --- | --- | --- |
| [libsodium-sys crate](https://crates.io/crates/libsodium-sys/0.2.7), including libsodium 1.0.18 | 0.2.7 | `6b779387cd56adfbc02ea4a668e704f729be8d6a6abd2c27ca5ee537849a92fd` | MIT or Apache-2.0 for bindings; ISC for libsodium |
| [libsodium-cmake](https://github.com/robinlinden/libsodium-cmake) | `a8ac4509b22b84d6c2eb7d7448f08678e4a67da6` | `8a722a7ef1ab8da12068cc1ff88d7727c6510d224f3d2addec4380de6c5c1266` | ISC |

The wrapper lists two `sandy2x` headers absent from the crate's bundled source.
The script removes only those two source-list entries from its cached copy.
It does not change libsodium's implementation. Static, full (non-minimal)
libraries are built because sodiumoxide exposes APIs beyond the minimal set.

For an OHOS Cargo target, set `SODIUM_LIB_DIR` to the corresponding build
directory before compiling `sodiumoxide`/`libsodium-sys`. The latter links
`static=libsodium` on an MSVC build host even for an OHOS target; the script
therefore creates an identical `liblibsodium.a` filename alias beside
`libsodium.a`. Other build hosts select `static=sodium`. Do not set `SODIUM_STATIC`: version
0.2.7 rejects that deprecated variable. On Windows/MSVC, libsodium-sys 0.2.7
uses its bundled `msvc/x64/{Debug,Release}/v142/libsodium.lib` by default.

Validation on DevEco Native SDK 6.1.1.125: both archives compiled, and their
objects report `EM_AARCH64` and `EM_X86_64`, respectively. Symbols including
`sodium_init`, `crypto_box_seal`, and `crypto_sign_ed25519_verify_detached`
are present. This does not replace an OHOS Rust link and device runtime test.
