# flutter_hbb

本目录是 RustDesk 现有 Flutter 桌面与移动工程。Open Remote Desk 的正式 Windows 被控工作台也在这里实现；Sciter 仅保留为此前已验证的开发壳，不作为正式桌面界面交付。

## Windows 正式被控工作台

专用窗口由 `--cm --ord-secure-ui` 进入，要求匹配的 `windows + flutter + ord-secure-host` Rust DLL；普通 CM 与 feature-off 保持原行为。

- `lib/desktop/widgets/secure_host_workspace.dart`：纯 Flutter 原型视图，接收不可变连接快照与按连接 ID 的回调；本机访问、当前运行记录、安全说明，以及固定的批准/拒绝/结束操作栏。
- `lib/desktop/pages/secure_host_page.dart`：适配现有 ServerModel/Flutter Rust Bridge、真实 CM 回执与窗口生命周期。提交本机批准后等待回执，不提前将界面置为已授权。
- `lib/desktop/secure_host_mode.dart`：显式专用模式标记；不把专用窗口行为套到普通 CM。

视频和键鼠默认关闭。`-Video` 开启视频入口，`-Input` 额外开启输入入口且要求同时传 `-Video`；这是本机实现能力门禁，不是第二次用户批准。Windows 每次连接只批准或拒绝一次，准入后控制端默认仅查看，并可选择启用或关闭键鼠；不再提供第二次“允许键鼠”操作。工作台只展示真实能力/键鼠状态，保留结束连接；控制端实际输入等待真实回执，不乐观启用。CM 连接批准不等于视频已协商或键鼠已启用；来源名称与 ID 为对方自报。文件、无人值守、协助码、音频、跨端剪贴板和提权未开放，主屏示意图不提供实时反馈。

控制协议令牌只在 Rust 中维护，每次启用重新生成；同一时刻只有一个输入拥有者。启用期间使用双向内部心跳，Windows 3 秒未收到有效输入/心跳会撤权清理，核心 5 秒无服务端消息会停止连接。关闭、取消或断开会清空待发输入，清理失败显式终止并封锁后续启用，需要重启被控进程。

## 构建与运行

使用固定 **Flutter 3.24.5 / Dart 3.5.4**、FRB codegen 1.80.1 和 cargo-expand 1.0.95，准备 MSVC/Windows SDK、Windows libclang 与仓库锁定的 `x64-windows-static` vcpkg 依赖。以下命令从 RustDesk 仓库根运行：

```powershell
& ./scripts/build-windows-host.ps1 -Configuration Debug -GenerateBridge
& ./scripts/build-windows-host.ps1 -Configuration Debug -Test
& ./scripts/start-windows-host.ps1 -Configuration Debug -Video
```

键鼠联调时，将启动命令改为 `./scripts/start-windows-host.ps1 -Configuration Debug -Video -Input`；先退出同端口的旧测试进程。启动后仍须本机批准每次连接，随后由鸿蒙控制端切换键鼠。

默认隔离 SDK 在外层工作空间 `.tools/flutter-3.24.5/flutter`；脚本支持 `-FlutterRoot` 指向其他已安装的同版本 SDK，并按 `pubspec.lock` 解析依赖。缓存完整时可传 `-Offline`。产物为 `flutter/build/windows/x64/runner/Debug/` 整包，不能用单独的 `target/debug/rustdesk.exe` 或 DEMO EXE 替代。

启动默认仅监听 `127.0.0.1:21120`，不传 `-Video` 时视频关闭，不自动批准、不修改防火墙或系统安全设置。详细前提、公开配置与鸿蒙连接步骤见 [正式入口说明](../docs/project/OFFICIAL-SECURE-ENTRY.md)。

## 当前验证范围

2026-10-04：Windows Debug 整包及鸿蒙双 ABI/HAP 构建通过并已安装，新 HAP 运行于手机模拟器；专用页面定向 analyze 无问题。自动化合计 **122 项通过**：core 84、旧 DEMO 9、Windows 15（模式选择 2、批准门禁 1、视频 5、输入 7）、Flutter widget 4、鸿蒙模型 10。单独执行视图检查时，在本目录使用同一固定 SDK：

```powershell
flutter --suppress-analytics --no-version-check analyze --no-pub lib/desktop/widgets/secure_host_workspace.dart test/desktop/secure_host_workspace_test.dart
flutter --suppress-analytics --no-version-check test --no-pub test/desktop/secure_host_workspace_test.dart
```

FRB、匹配的 Rust DLL 与 Flutter Windows Debug 整包已构建成功并实际打开；本机访问、访问记录、安全设置及最大化/还原已有截图检查。用户本机批准后，Flutter 显示“连接已获本机批准”，活动记录按提交批准、被控端确认的顺序更新；手机模拟器通过原生 XComponent 显示真实 Windows 主屏 1280×720。Flutter 主动结束连接、手机清屏、重新连接后重新审批与不显示旧画面均已实测；待批准时关闭工作台也已确认连接失败并清屏。等待批准会创建待显示的 XComponent，不能把组件存在当成已经显示视频。

前轮拒绝按钮与活动视频直接关窗已补验通过，详见 [Flutter 界面记录](../docs/project/research/2026-10-03-windows-host-ui-validation.md)。v2 已现场验证一次接入批准、控制端开启、确认文本到达记事本和切回仅查看；鼠标、滚轮和物理按键仍待单独验收。 新的 Desk 使用 `secure_control` 和 `input_version=2`，旧 Index 的 `secure_video`/DEMO 保留；旧 host 能力不匹配时失败，不自动降级。仅 control 路径在 Windows GDI 叠加真实光标。当前证据见 [能力切换验证记录](../docs/project/research/2026-10-04-controller-capability-validation.md) 与 [PROGRESS](../docs/project/PROGRESS.md)；真机、公网、硬件解码等仍未验收。
