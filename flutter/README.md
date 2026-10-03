# flutter_hbb

本目录是 RustDesk 现有 Flutter 桌面与移动工程。Open Remote Desk 的正式 Windows 被控工作台也在这里实现；Sciter 仅保留为此前已验证的开发壳，不作为正式桌面界面交付。

## Windows 正式被控工作台

专用窗口由 `--cm --ord-secure-ui` 进入，要求匹配的 `windows + flutter + ord-secure-host` Rust DLL；普通 CM 与 feature-off 保持原行为。

- `lib/desktop/widgets/secure_host_workspace.dart`：纯 Flutter 原型视图，接收不可变连接快照与按连接 ID 的回调；本机访问、当前运行记录、安全说明，以及固定的批准/拒绝/结束操作栏。
- `lib/desktop/pages/secure_host_page.dart`：适配现有 ServerModel/Flutter Rust Bridge、真实 CM 回执与窗口生命周期。提交本机批准后等待回执，不提前将界面置为已授权。
- `lib/desktop/secure_host_mode.dart`：显式专用模式标记；不把专用窗口行为套到普通 CM。

视频默认关闭，只有本机显式 `-Video` 后才允许请求只读画面，每次连接仍需现场批准。CM 的批准回执同时适用于非媒体连接，不能据此宣称视频已协商或正在共享屏幕；视图说明视频还需本机启用并完成本次协商。来源名称与 ID 为对方自报，不能视为可信控制设备身份。键鼠、文件、无人值守、协助码、音频、剪贴板和提权未开放；主屏示意图不提供实时反馈。

## 构建与运行

使用固定 **Flutter 3.24.5 / Dart 3.5.4**、FRB codegen 1.80.1 和 cargo-expand 1.0.95，准备 MSVC/Windows SDK、Windows libclang 与仓库锁定的 `x64-windows-static` vcpkg 依赖。以下命令从 RustDesk 仓库根运行：

```powershell
& ./scripts/build-windows-host.ps1 -Configuration Debug -GenerateBridge
& ./scripts/build-windows-host.ps1 -Configuration Debug -Test
& ./scripts/start-windows-host.ps1 -Configuration Debug -Video
```

默认隔离 SDK 在外层工作空间 `.tools/flutter-3.24.5/flutter`；脚本支持 `-FlutterRoot` 指向其他已安装的同版本 SDK，并按 `pubspec.lock` 解析依赖。缓存完整时可传 `-Offline`。产物为 `flutter/build/windows/x64/runner/Debug/` 整包，不能用单独的 `target/debug/rustdesk.exe` 或 DEMO EXE 替代。

启动默认仅监听 `127.0.0.1:21120`，不传 `-Video` 时视频关闭，不自动批准、不修改防火墙或系统安全设置。详细前提、公开配置与鸿蒙连接步骤见 [正式入口说明](../docs/project/OFFICIAL-SECURE-ENTRY.md)。

## 当前验证范围

最新版本已使用 Flutter 3.24.5 完成专用模式/页面/视图定向 analyze（无问题）和 3 组 widget tests：按目标 ID 操作、批准等待回执、多请求与禁用功能、宽/窄/短窗口及滚动后的固定按钮。Rust 的 2 项模式选择、1 项批准门禁与 4 项视频测试也已通过。单独执行视图检查时，在本目录使用同一固定 SDK：

```powershell
flutter --suppress-analytics --no-version-check analyze --no-pub lib/desktop/widgets/secure_host_workspace.dart test/desktop/secure_host_workspace_test.dart
flutter --suppress-analytics --no-version-check test --no-pub test/desktop/secure_host_workspace_test.dart
```

FRB、匹配的 Rust DLL 与 Flutter Windows Debug 整包已构建成功并实际打开；本机访问、访问记录、安全设置及最大化/还原已有截图检查。真实手机连接已到达待批准状态，用户本机批准、真实视频与后续断开/重连清理仍待现场验收，不能从 widget tests 或请求到达推断通过。完整证据见 [Windows Flutter 验证记录](../docs/project/research/2026-10-03-windows-host-ui-validation.md)，当前状态以 [PROGRESS](../docs/project/PROGRESS.md) 为准。真机、公网和系统输入均未验收。
