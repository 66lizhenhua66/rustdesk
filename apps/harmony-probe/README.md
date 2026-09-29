# 鸿蒙原生桥接验证工程

该工程验证 ArkUI → C++ NAPI → Rust C ABI → 后台回调，以及取消/释放/重新进入的生命周期。Rust crate 没有第三方依赖，也没有接入 RustDesk 核心、远控网络、屏幕采集或生产权限体系。

已验证：DevEco SDK 6.1.1.125/API24 构建，兼容目标 API22，x86_64/API22 模拟器运行；ARM64 仅完成交叉编译和链接。见 [验证报告](../../docs/project/research/2026-09-30-harmony-bridge-validation.md)。

## 构建

需要已有 DevEco Studio、Rust 和以下标准库目标：

```powershell
rustup target add x86_64-unknown-linux-ohos aarch64-unknown-linux-ohos
cargo test --manifest-path apps/harmony-probe/native/rust/Cargo.toml --locked
& ./apps/harmony-probe/scripts/build.ps1 -DevEcoRoot 'G:\Huawei\DevEco Studio'
```

命令从 RustDesk 仓库根目录执行。脚本显式选择 DevEco 的 Node/Java/Hvigor/SDK，避免 PATH 中其他 SDK 干扰。Rust 生成 staticlib，最终由鸿蒙 CMake 链接 NAPI `.so`。实际验证使用 Rust 1.96.1、Hvigor 6.24.4；还未测试更老版本。

HAP 输出：`entry/build/default/outputs/default/entry-default-unsigned.hap`，含 x86_64 和 arm64-v8a 两种 native 库。工程不包含应用签名或身份凭据。

## 模拟器安装和验证

先确认目标设备是自己的测试模拟器，且没有同包名的重要应用。本工程包名 `com.openremotedesk.bridgeprobe`。

```powershell
$probeHdc = 'G:\Huawei\DevEco Studio\sdk\default\openharmony\toolchains\hdc.exe'
& $probeHdc list targets
& $probeHdc -t 127.0.0.1:5555 install ./apps/harmony-probe/entry/build/default/outputs/default/entry-default-unsigned.hap
& ./apps/harmony-probe/scripts/verify-emulator.ps1 -Device '127.0.0.1:5555'
```

当前模拟器实际接受 unsigned HAP；这不是所有设备的保证。鸿蒙 6.1 真机安装需要本机有效调试签名/授权与设备连接，不能把模拟器接受未签包的行为外推到真机。

应用启动自动检查版本、参数拒绝、异步完成、取消无后续进度、替换无旧回调、重复释放后重新开始。验证脚本在 UI 层另检查后台任务可取消、前后台恢复后旧超时不影响新任务、三轮退出重进，并收集当前应用标签日志与截图。

运行证据保存在被忽略的 `artifacts/`。脚本不会自动安装最新 HAP，改代码后必须先重新构建并安装；它也不验证内存长期趋势、横竖屏、视频或真实远控。

## 模块与资源规则

- `native/rust`：纯 Rust 模拟任务，通过 Condvar 及时取消，三个行为测试。
- `entry/src/main/cpp`：每个 NAPI 环境一个任务；TSFN 分别持有 worker/owner 引用；停止顺序为取消、等待 worker 退出、关闭 TSFN、销毁 Rust 对象。
- `entry/src/main/ets`：页面代次隔离；页面隐藏/销毁停止当前任务；异步返回与超时只对所属代次生效。
- `scripts`：构建与当前测试模拟器的可重复验收入口。

本例 stop 在 UI 线程 join 的前提是测试 worker 只做可立即唤醒的本地等待，不依赖 JS 回调或网络。真实 RustDesk 网络/媒体核心必须使用非阻塞关闭与完成通知，不能直接沿用这个同步 join 当生产设计。

没有修改 RustDesk 现有 `src/`、`libs/` 或根 Cargo workspace；该 crate 使用独立 workspace，避免原生桥接试验拉入全部远控依赖。
