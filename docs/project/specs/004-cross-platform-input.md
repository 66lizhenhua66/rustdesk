# SPEC-004：跨平台控制端输入

日期：2026-10-07。用户要求输入解耦面向后续其他移动端和 Windows。采用既定 ArkUI 与 Flutter 分端界面、共享 Rust 核心路线，细化 [SPEC-002](002-controller-tech-stack.md)。

输入主链路、系统快捷键与输入法分流见 [控制输入流向图](../INPUT-FLOW.md)。

## 分层与角色

| 层 | 实现位置 | 责任 |
| --- | --- | --- |
| 平台事件适配 | 鸿蒙 `HarmonyInputAdapter.ets`；Flutter `flutter_input_adapter.dart` | 获取触点、鼠标按钮、滚轮、物理键和已确认文本；转换平台键码；焦点和系统输入法接线 |
| 共享交互引擎 | `libs/controller-core/src/interaction.rs` | 手势识别、画面坐标、滚轮余量、按钮/物理键保持、同语义修饰键释放、Unicode 文本分块 |
| 跨语言入口 | `libs/controller-core/include/input.h` | 版本化 C ABI、标准事件、同步命令 sink、明确对象与缓冲区生命周期 |
| 会话与授权 | 已有 `controller_session_send_input_v1` / `set_input_enabled_v1` | 真实授权、令牌、队列、撤权与失联清理；交互引擎不能自行授予权限 |
| 被控平台执行 | 当前 Windows `secure_input.rs` | 执行语义命令、核查交互桌面并在撤权时释放；与 Windows 控制端事件采集分开 |

鸿蒙的 `SessionInputController` 只负责本地模式选择、会话状态呈现和原生桥调用，不再实现手势算法；其 `ScreenState`、中文提示和系统键盘不进入共享 Rust 模块。后续 Android/iOS/Windows 的 UI 可采用各自布局和输入法，但不能另抄一套手势或按键状态机。

Windows 控制端采用 Flutter 事件适配，经同一 Rust 引擎产生远程命令；Windows 被控端接收命令后经既有执行器注入。这两个角色可以存在于同一桌面应用，接口与生命周期独立。

## 标准事件与 ABI

每个控制会话持有一个 `ControllerInput` 不透明句柄。事件种类为 `touch`、`mouse`、`wheel`、`key`、`text`、`release`，完整字段见 `input.h`。坐标使用实际远程图像内的逻辑单位，不包含黑边；宽高与事件坐标单位一致。共享引擎归一化到 0—65535。时间是非负毫秒，物理键 ID 只标识来源，语义码决定远端行为。

- `controller_input_new_v1` 创建；`event_v1` 同步处理标准事件；`reset_v1` 只清本地状态；`free_v1` 销毁。
- 单个 JSON 事件最多 8192 UTF-8 字节、每个触点列表最多 16 项。文本命令按完整 Unicode 码点分成不超过 512 UTF-8 字节，NUL 不允许。
- sink 按顺序接收命令，返回 0 表示已接受；非 0 立即停止余下命令并清理引擎状态。成功只表示入队，不表示远端已经执行。
- sink 必须调用真实会话门禁。鸿蒙 NAPI 出错后直接关闭会话输入，失败则取消连接；Dart 包装要求 `onFailure` 执行同样的关闭流程。
- 句柄单线程使用，回调期间不能重入同一引擎。命令字符串只在回调内借用，Dart 立即复制解析。退出前先撤权/断连，再清理和销毁本地句柄。

引擎不包含连接地址、授权令牌、ArkUI 对象、Flutter 对象、窗口句柄或 UI 文案。扩展触控模式、长按/拖动策略、滚轮或组合键时优先修改 Rust 引擎，并用公开 ABI 的命令序列测试固定行为；平台特有键码、IME 和生命周期只在 Adapter 改动。

## 当前接入状态与后续门槛

| 目标 | 本轮结果 | 仍需完成 |
| --- | --- | --- |
| 鸿蒙控制端 | 正式 NAPI 路径已调用共享引擎；双 OHOS ABI/HAP 构建与模拟器安装启动通过 | 新链路真实远程输入、HarmonyOS 真机与输入法回归 |
| Windows Flutter 控制端 | Dart FFI 与 Flutter 事件 Adapter 已实现，加载真实 Windows DLL 的行为测试通过 | 接入完整控制端会话 UI、随应用打包库、桌面键鼠现场验证 |
| Android Flutter 控制端 | 共用上述 Dart/Flutter 接口，Rust/C ABI 无鸿蒙对象依赖 | Android 各 ABI 原生构建及打包、IME/生命周期接线、真机验证 |
| iOS/iPadOS Flutter 控制端 | 共用上述 Dart/Flutter 接口，可用静态库经进程符号访问 C ABI | macOS/Xcode 工具链、静态链接与符号保留、签名、IME/生命周期和真机验证 |
| Windows 被控端 | 继续使用严格输入执行器，接收相同语义命令 | 按被控端安全与输入清单继续现场验收 |

库产物不等于完整多端应用。本轮 Windows DLL 测试证明 Dart 能真实调用同一 Rust 引擎，不替代 Android/iOS 编译或真实远控结果。

## 构建与验证

默认保留 `controller-core` 的 `staticlib/rlib`，以兼容既有 OHOS NAPI 静态链接。需要 Windows 测试 DLL 时显式选择动态库：

```powershell
cargo test --manifest-path libs/controller-core/Cargo.toml --locked --offline
cargo rustc --manifest-path libs/controller-core/Cargo.toml --lib --crate-type cdylib --locked --offline
$env:CONTROLLER_INPUT_LIBRARY = (Resolve-Path 'libs/controller-core/target/debug/remote_controller_core.dll').Path
Push-Location flutter
& '..\..\.tools\flutter-3.24.5\flutter\bin\flutter.bat' test --no-pub test/controller_input_ffi_test.dart
Pop-Location
```

Flutter 接入与失败关闭契约见 [包装说明](../../../flutter/lib/controller_input/README.md)。未来平台发布要增加各自打包步骤，不使用 Windows 测试路径作为运行时默认路径。

## 本轮实现回归面

- `controller_napi.cpp` / `Index.d.ts`：增加标准事件与 reset 入口，Job 持有共享引擎；逐命令 sink 进入既有授权通道，错误时关闭输入，是鸿蒙使用同一 Rust 实现的必要接线。
- `Desk.ets` / `RemoteSession.ets`：传递标准事件、清理通知和 pre-IME 事件，模式与实际授权门禁继续生效。
- `SessionInputController.ts` / `InputEvents.ts` / `InputModels.ts`：移除本地手势、坐标与文本算法，保留本地界面状态与事件契约；原 `TouchInput.ts` 和 `MouseKeyboardInput.ts` 已迁入 Rust。
- `RemoteInputMethod.ets`：确认文字直接交给共享引擎分块，不再在鸿蒙重复维护 Unicode 拆包算法。
- `libs/controller-core/src/lib.rs`：注册独立 `interaction` 模块；新 ABI 只产生语义命令，不改既有协议或授权实现。
- `flutter/lib/controller_input/`：新增独立包装与事件适配，不改既有 Flutter 远控页面或 Windows 被控执行器。

本轮针对上述路径通过 Rust 核心 111 项、鸿蒙模型/接线 20 项、Flutter 真实 DLL 3 项测试。OHOS 双 ABI、HAP、Windows FFI DLL 构建通过，模拟器安装启动通过；系统拦截权限、Alt+Tab 捕获和星闪真机行为仍待验证。
