# T0/T1 鸿蒙桥接验证结果

日期：2026-09-30。实施基线：`42aa3a9c3`；分支：`feat/harmony-controller`。范围：[T0/T1 计划](../plans/2026-09-30-harmony-bridge-probe.md)。结论：独立 ArkUI/NAPI/Rust 桥接在当前模拟器通过，尚未证明官方 RustDesk 核心可在鸿蒙运行。

## 实际环境

| 项目 | 结果 |
| --- | --- |
| DevEco 安装 | `G:\Huawei\DevEco Studio`，IDE build 243.24978.46.36.611300 |
| 构建 SDK | 内置 HarmonyOS 6.1.1.125，API24，包含 native 工具链 |
| 工程 target / compatible | `6.1.1(24)` / `6.0.2(22)` |
| Hvigor / ohpm | 6.24.4 / 6.1.2.285，使用 IDE 内置 Node |
| Rust / Cargo | 1.96.1 / 1.96.1 |
| 交叉目标 | x86_64-unknown-linux-ohos、aarch64-unknown-linux-ohos（本次补装标准库） |
| 运行设备 | 用户启动的本机模拟器，x86_64、API22、OpenHarmony-6.0.2.130 |
| 真机 | 用户具备鸿蒙 6.1 真机，本次没有连接或运行验证 |

系统 PATH 的 hdc 来自 API20 SDK，独立 command-line-tools 默认 SDK 为 API22；验证脚本显式选择 DevEco SDK，避免混用。模拟器系统为 API22，不因用 API24 SDK 构建就变成鸿蒙 6.1 真机测试。

## 结果与证据

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| Rust 正常完成、取消唤醒、参数拒绝 | 3/3 通过 | `cargo test --manifest-path apps/harmony-probe/native/rust/Cargo.toml --locked`，取消等待测试上限 250ms |
| Rust 格式 | 通过 | `cargo fmt --manifest-path apps/harmony-probe/native/rust/Cargo.toml --check` |
| 双 ABI 编译与链接 | 通过 | Rust staticlib + CMake 生成 x86_64、arm64-v8a 的 libprobe.so；readelf Machine 分别为 X86-64 / AArch64，具有 Rust probe_run/probe_version 与 NAPI 注册符号 |
| ArkTS 与 HAP | BUILD SUCCESSFUL | DevEco Hvigor assembleHap；unsigned 包包含双 ABI，无签名材料 |
| 模拟器安装/启动 | 通过 | HDC install 返回 install bundle successfully；aa start 成功，UI 与应用日志显示结果 |
| 应用内自动断言 | 6/6 通过 | 版本、错误参数、异步完成、取消无后续进度、替换无旧回调、重复释放后重启 |
| 真实 UI 取消 | 通过 | 任务未完成时点击取消，进度停在 21，后续读取保持不变（具体进度随调度变化） |
| 前后台恢复 | 通过 | 自动验证中切 Home，再恢复；新任务跨过旧 3 秒超时窗口后仍持续增加进度 |
| 页面退出/重进 | 3/3 轮通过 | 每轮任务运行时 Back，再启动；页面再次显示 6/6 |
| 独立审阅 | 已修正后通过 | 修复旧 timeout 可停止新页面任务，以及 await 后失效校验遗漏；再次只读审阅无必须修正项 |

设备日志片段（BridgeProbe 标签）：

```text
PASS version
PASS invalid_arguments
PASS async_completion
PASS cancel_no_late_progress
PASS replace_no_stale_callback
PASS dispose_and_restart
RESULT 6/6
```

本机构建 HAP SHA-256：`873A347D295BF3D96F7F00A8D004C37BC3CB0D8FB82DAF8970B0BE4B843D675D`（对应首次通过上述设备验收的构建；后续重建可能变化）。日志、布局、截图保存在 `apps/harmony-probe/artifacts/`，不进入 Git；工程和重现脚本进入 Git。

## 调试中实际修正

1. IDE `ohpm.bat` 在本次调用中出现批处理递归溢出，改用 IDE Node 直接调用其 `pm-cli.js`。
2. `DEVECO_SDK_HOME` 应为 IDE 的 `sdk` 根目录，不能多加 `default`。
3. Hvigor 传入 `OHOS_ARCH=arm64-v8a`，CMake 同时识别该值和 arm64。
4. 生命周期审阅发现旧超时和异步链可能重启/关闭非所属页面任务；新增 page hide 和每轮 epoch 约束后通过前后台/退出测试。
5. 模拟器截图工具只接受 JPEG，脚本按实际要求修正，并检测 HDC 返回文本中的错误，避免只信退出码。

## 证据边界与下一步

- 这是一套模拟任务桥接工程，没有复制 RustDesk 协议实现，也不需要开放网络端口。
- 已证明当前工具链的原生编译、链接、ArkTS 调用、线程回调和有限生命周期用例可用；没有证明正式客户端、完整 SDK 或长期稳定性。
- 未测试鸿蒙 6.1 ARM64 真机安装/签名、横竖屏、长期内存趋势、视频硬解、音频、中文输入、RustDesk ID/IP/中继、认证和权限。
- 示例同步 join 只等待可立即取消的本地任务；生产网络/媒体关闭必须按 SPEC 使用异步关闭完成契约。
- 下一步 T2 先做官方 RustDesk controller-only 的依赖/条件编译清单和最小编译尝试，确定可复用会话边界，再接真实连接；不能以本次 6/6 代替它。

## 官方 API 依据

已通过华为知识 MCP 检索并阅读 Node-API 线程安全、清理钩子与内存管理资料：

- [使用 Node-API 接口进行线程安全开发](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/use-napi-thread-safety)
- [Node-API 支持的数据类型和接口](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/napi-data-types-interfaces)
- [内存泄漏相关问题汇总](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/napi-faq-about-memory-leak)
- [注册和使用环境清理钩子](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/use-napi-about-cleanuphook)

TSFN 基础接口自 API10、环境清理钩子自 API11；后台不跨线程使用 napi_env/napi_value，投递失败释放数据，停止生产后才关闭 TSFN。本报告不是完整安全审计。
