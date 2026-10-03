# 官方 Windows 安全入口

本阶段把安全准入代码放入官方被控端的 `Connection` 和连接管理器（CM）路径。它与 `apps/windows-demo-host` 是两个不同的程序。默认构建保持上游行为；产品专用构建必须使用 `ord-secure-host` feature。安全入口默认仅非媒体会话，可显式开启现场批准后的主屏只读视频；系统输入、文件、音频、终端、剪贴板、提权和无人值守仍不开放。

## 正式 Flutter 工作台（2026-10-03）

正式 Windows 桌面使用现有 `flutter/` 工程。`--cm --ord-secure-ui` 选择原型对应的本机访问工作台，并要求匹配的 `windows + flutter + ord-secure-host` Rust 库；专用 IPC 与旧 CM 分离，普通 Flutter CM、Sciter 开发壳和 feature-off 保持原路径。`target/debug/rustdesk.exe` 是此前的 Sciter 开发壳，不是本轮 Flutter 交付包。

界面包括本机设备 ID、来源名称与自报 ID、逐连接批准/拒绝、连接及结束状态、当前运行的访问记录与安全说明。点击“批准连接”后先显示正在确认，收到服务端回执后才显示本次连接已批准。CM 的批准回执没有视频协商或首帧字段，非媒体请求也能获批，因此界面始终说明屏幕共享仍需本机启用视频并完成本次协商；显示器只表示可能的共享范围，不提供实时画面反馈。来源名称与 ID 不等于已核验的控制设备身份；无人值守、可信设备登记、协助码、键鼠与文件等未实现能力禁用。

FRB、匹配的 Rust DLL 与 Flutter Windows **Debug 整包已构建成功并实际运行**。最新 3 组 widget tests、专用模式/页面/视图定向 analyze，以及 2 项模式选择、1 项批准门禁、4 项视频回归均通过；本机访问、访问记录、安全设置三页及窗口最大化/还原已截图检查。用户本机批准后，Flutter 收到批准回执，手机模拟器通过原生 XComponent 显示真实 Windows 主屏 1280×720；本次观察接收/呈现 2037/2032 帧、9816 KiB。Flutter 主动结束连接后手机清屏，重连重新进入待批准且不显示旧画面；待批准阶段关闭 Flutter 窗口也已验证连接失败并清屏。**有视频时直接关窗、拒绝按钮实际点击、真机、公网与系统输入仍未验收**。详见 [Windows Flutter 验证记录](research/2026-10-03-windows-host-ui-validation.md) 与 [PROGRESS](PROGRESS.md)。

准备 Flutter 3.24.5（Dart 3.5.4）、`flutter_rust_bridge_codegen` 1.80.1、`cargo-expand` 1.0.95、MSVC/Windows SDK、Windows libclang，以及仓库固定 baseline 的 `x64-windows-static` vcpkg 依赖。从仓库根运行：

```powershell
& ./scripts/build-windows-host.ps1 -Configuration Debug -GenerateBridge
& ./scripts/build-windows-host.ps1 -Configuration Debug -Test
& ./scripts/start-windows-host.ps1 -Configuration Debug -Video
```

构建脚本默认使用外层工作空间的 `.tools/flutter-3.24.5/flutter`，通过 `-FlutterRoot`、`-PubCache`、`-BridgeTools`、`-VcpkgRoot`、`-LibClangPath` 可指定已有工具位置；依赖缓存完整后才使用 `-Offline`。脚本按 `pubspec.lock` 解析依赖，不使用全局最新版 Flutter。产物目录为 `flutter/build/windows/x64/runner/Debug/`；EXE、`librustdesk.dll`、`flutter_windows.dll` 和资源目录需保持同包。Release 对应 `-Configuration Release` 与同名输出目录。

启动脚本分别启动严格服务与可见的 Flutter 工作台，默认监听 `127.0.0.1:21120`，公开配置输出到 `apps/harmony-controller/artifacts/official-current-profile.json`。不传 `-Video` 时视频关闭；工作台不会自动批准请求，批准后保持可见，末条会话结束后保留工作台。待批准连接在超时前关闭工作台后，手机立即连接失败并清屏，XComponent 数为 0；这项实测不覆盖有视频时直接关窗。

## 只读画面与鸿蒙连接

完成上述 Flutter 整包构建后，在鸿蒙 Desk 工作台导入并核对公开配置，选中设备后点击“连接桌面”，再在 Windows 工作台点击“批准连接”。模拟器回环转发使用 `hdc rport tcp:21120 tcp:21120`。启动前应退出同一测试入口的旧进程，避免端口占用；脚本不代替本机用户批准。

`-Video` 显式设置 `ORD_SECURE_VIDEO=1`，不传则设为 0；只有控制端请求 VP8、服务端本地开启、CM 批准三者满足时才启动采集。使用 Windows GDI 主屏采集和 VP8 软件编码，最大 1280×720、约 8 fps；相同像素不重复发送，变化帧使用 VP8 帧间压缩。这不是任意应用串流，也不承诺只发送脏矩形、GPU 硬解或公网性能。

鸿蒙保留的诊断页中，“连接正式入口”仍是原非媒体诊断；“查看桌面（只读）”明确绑定 `secure_video`。断开、应用进入后台或 Surface 销毁会取消视频；正式工作台的“返回设备”保留当前 Surface 和会话。锁屏/安全桌面/捕获失败关闭会话，不能用测试图替代。界面触摸不注入 Windows 输入。此前开发壳的真实只读画面证据见 [只读画面记录](research/2026-10-02-readonly-screen-validation.md) 与 [鸿蒙正式界面验证](research/2026-10-03-native-controller-ui-validation.md)，不能作为新 Flutter 被控端已验收的证据。

## Sciter 开发壳构建与历史证据

先使用仓库 `vcpkg.json`、overlay 与固定 baseline 安装 `x64-windows-static` 依赖（与 `.github/workflows/flutter-build.yml` 的 Windows 环境一致），准备 MSVC/Windows SDK 和 Windows libclang。然后：

```powershell
& ./scripts/build-official-secure-host.ps1 -VcpkgRoot 'C:\vcpkg' -CheckOnly
& ./scripts/build-official-secure-host.ps1 -VcpkgRoot 'C:\vcpkg'
```

本工作空间已将锁定依赖安装到忽略的 `apps/harmony-controller/artifacts/official-build/vcpkg`，脚本可自动发现这个缓存，因此本机可直接运行 `-CheckOnly`、`-TestSecureGate` 或 `-DebugBuild`。脚本自动加载 MSVC 开发环境并指定 bindgen 的 MSVC 目标/Clang 资源目录，结束时恢复进程环境。其他机器仍需准备自己的 vcpkg 依赖；不提交缓存或修改全局环境。

路径为示例；本机 DevEco 的 libclang 可以复用，但它不提供官方 Windows RustDesk 所需的 vcpkg 媒体库。该脚本用于此前的 Sciter 开发壳与既有核心回归，不生成本轮正式 Flutter 工作台；正式桌面使用上文的 `build-windows-host.ps1`。

当前实际验证结果以 [PROGRESS](PROGRESS.md) 和本轮验证报告为准。独立策略测试通过不等于完整官方 EXE 已成功编译或真实 CM 已完成运行验证；不要将上一轮 DEMO EXE 重命名为正式端来替代。

2026-09-30 已完成 feature-off/on 的完整 Windows 编译检查和开发版链接，实际程序为 `target/debug/rustdesk.exe`，同目录有 Sciter 运行时。当时尚未验收真实 CM 与 GUI，详见 [历史记录](research/2026-09-30-official-secure-entry-validation.md)；后续批准与只读画面证据以上述 2026-10-02/03 记录为准。

## 开发壳显式启动与通用门禁

2026-10-02 的安全构建 Debug CM 修复可继续通过 PowerShell 7 的 `./scripts/start-official-secure-host.ps1` 复测；显式 `-Video` 才开启只读视频。该脚本启动 Sciter 开发壳的 `--server` 和 `--cm`，默认仅监听本机 21120，不自动批准。它不替代正式 Flutter 的 `start-windows-host.ps1`。

在具备正确构建及运行时依赖的机器上，通过本地进程环境开启监听。以下是预期运行步骤，未替代本机完整集成验收：

```powershell
$env:ORD_SECURE_LISTEN = '127.0.0.1:21120'
$env:ORD_SECURE_PROFILE_OUT = "$PWD\secure-host-public.json"
& ./target/release/rustdesk.exe
```

不设 `ORD_SECURE_LISTEN` 时安全构建不监听；其被控启动流程不再启动公共 rendezvous、旧 direct 或 LAN 服务。地址必须为具体 IP 和非零端口；非 loopback 还必须设置逗号分隔的 `ORD_SECURE_ALLOW` 来源 IP（最多 16 个），不接受通配来源。不会自动开防火墙。身份使用本机 Config 已有密钥，公钥导出只含可保存的公开字段；拒绝缺失/不匹配身份，不通过降级保证连接成功。

仍遵守正式程序的 `stop-service` 开关：已停用则不绑定；运行时停用会关闭监听并结束安全会话。监听/空闲会话每秒检查，正在进行的握手和写入还受各自超时约束，因此不是硬性的“一秒断开”承诺。本切片停止后重新开启需重启该进程；暂不实现设置热重载。

使用鸿蒙“导入公开配置”后，点击“连接正式入口”，等待官方 CM 的现场批准。该按钮显式发送 expectedPeer=secure_host；“连接演示端”只接受 DEMO，不能自动切换。正式连接始终不开放输入，任何权限提升通知都会断开。旧“验证加密登录”仍为成功后立即关闭的单次诊断。

## 访问边界

- 握手要求 TCP、有效签名身份和密钥交换 v1。空公钥更新、v0、未知版本和非预期消息直接拒绝。
- 仅接受一次普通远程登录请求；只读画面仅识别 VP8 解码能力和明确的 VP8 偏好，其余 `option`、客户端版本/平台元数据不调用旧选项处理；拒绝 OS 凭据、文件/摄像头/终端/隧道登录及可信设备标签。
- 本切片只允许空密码 + 本地批准。已有密码自动批准、最近会话复用、切换控制方向都不可达；配置了 2FA 则拒绝本轮连接，不把点击批准当作绕过必要因子的许可。
- 非媒体批准发送 `ord_secure_host=1, media=false, input_scope=none`；画面模式批准后发送 `media=true, video_codec=vp8` 及单个缩放显示尺寸。两者都发送全 false 的操作权限快照；批准前不采集、不发送显示资料。本地权限开关不能开放系统输入。
- 保持请求大小上限、既有未认证连接预算及有限会话寿命。下一阶段开放视频/键鼠时必须分别接入执行点授权，并补动态撤权/队列清理验收。

现有上游 Stream 对极短密文存在历史透传兼容逻辑。本轮在安全会话中保守拒绝解码后不超过 16 字节的消息，防止短明文被当作加密指令；这也会关闭使用极短合法心跳/关闭包的连接。当前鸿蒙单次登录请求和正常关闭说明不受该长度限制影响；未来持久正式会话接入时，需要专门完善这个传输边界并做兼容测试，不能移除检查后直接开放操作。

本阶段是安全入口工程接入，不是已完成公网产品。主 SPEC 的“短期协助凭据且现场批准”、完整可信控制身份与审计等门槛继续保留。
