# 正式主屏键鼠切片验证

日期：2026-10-04。实施基线 `c65fda879`，计划见 [正式主屏键鼠实施计划](../plans/2026-10-03-system-input.md)。本记录区分源码、自动化、构建和现场授权后的实测。

后续更新：用户已在旧流程点击允许键鼠；控制端实际显示“仅查看 · 已暂停键鼠”，且“控制”按钮可用，确认已收到输入许可但仍处于本地查看模式。随后用户要求被控端只批准接入、能力开关归控制端，正在按 [新流程计划](../plans/2026-10-04-controller-capabilities.md) 调整。下面保留 `499b89441` 切片当时的构建/测试快照，不把状态获准当作实际系统输入已验证。

## 当前结果

Windows 严格输入适配、Flutter 独立许可、Rust 共享协议/C ABI 与鸿蒙原生输入界面已实现。Windows Debug 整包和鸿蒙 x86_64/aarch64 双 ABI HAP 构建通过，新 HAP 已覆盖安装到 API22 手机模拟器，原设备资料仍在。PC 模拟器保持关闭。

新版真实连接已到达 Flutter 待批准窗口；截至本记录，尚未完成用户本机“批准连接 → 允许键鼠”。请求未获批准后超时，未执行真实系统输入。因此鼠标、中文文本、实际撤权与按住状态释放均不宣称现场验收通过。用于后续验证的专用文件为本机忽略目录 `apps/harmony-controller/artifacts/input-validation-20261004.txt`，不会以其他应用数据作为测试目标。

上一界面切片的拒绝和活动视频关窗已实测收口，见 [原记录补充](2026-10-03-windows-host-ui-validation.md)，不能据此替代本切片的系统输入验收。

## 实现与默认边界

- `-Video -Input` 显式允许运行时提供键鼠能力；不传 `-Input` 时 `ORD_SECURE_INPUT=0`。已有本机 keyboard 权限上限仍有效。查看批准、键鼠支持与本次键鼠许可分别处理。
- `secure_control` 与版本 1 的新增 protobuf 消息显式协商。旧 `secure_video`、非媒体严格入口和 DEMO 继续保留原路径；旧 host 不支持新字段时拒绝，不自动降级成输入会话。
- 每次本机授予使用随机 16 字节令牌，令牌只在 Rust 核心和加密网络中使用，不交给 ArkUI/NAPI。每个输入事件在 Windows 执行点再次验证当前令牌、本机上限、交互桌面和单连接输入拥有者。
- 严格适配器同步调用 SendInput，不使用旧无界输入队列、portable service、提权、SAS 或安全桌面切换。鼠标限制主屏；相对移动读取真实 Windows 光标位置，新控制视频路径叠加真实系统光标和缩放热点。
- 客户端有界队列，释放命令优先；撤权清令牌/队列，重授不接受旧令牌。Windows 先失效许可再释放本连接持有的键/按钮。授予强制远端点击保护；撤权与结束不被持续远端输入阻塞。
- 授权时每秒双向 KeepAlive。Windows 3 秒未收到有效令牌活动，在秒级检查中撤权并结束；核心在输入获准时 5 秒无有效入站即退出。未获输入授权的静态视频不触发该限制。控制写操作有 2 秒截止时间。
- SendInput 部分失败按实际成功数量记账。释放失败会明确显示 `INPUT_RELEASE_FAILED` / 本机失败提示，并保留输入拥有者以拒绝新授权，直到进程重启；不宣称已全部释放。
- 鸿蒙确认文本才发送，输入法预编辑不发送；相对触控板、明确按钮/拖动、滚轮及受限物理键通过低频命令接入。唯一原生 Surface 保留，像素不经过 ArkTS/JSON。

## 自动化与构建

本轮自动化 **116 项通过**，其中共享核心、DEMO 和界面模型回归均包含原有行为。

| 范围 | 结果 |
| --- | --- |
| controller-core 全套 | 80：lib 22、ABI 8、identity 3、official_host 7、session 40；含旧模式、令牌、队列饱和释放、双向心跳、静态只读、专用错误码 |
| 独立 Windows DEMO | 9：原 lib 4 与 session 5；输入仍只作用于演示窗口 |
| Windows Flutter 严格入口 | 14：模式选择 2、批准门禁 1、视频/光标 5、输入模拟注入器 6；测试不操作用户真实桌面 |
| Flutter widget | 4：新增本机支持/授权回执/撤权目标，原批准、关闭、宽窄短窗口回归 |
| 鸿蒙状态与输入模型 | 9：真实状态投影、输入可见性/许可、相对增量边界、文本确认条件 |
| Flutter 定向 analyze | 专用 mode/page/workspace 无问题；core fmt 与 diff 检查通过 |
| Windows Debug | `scripts/build-windows-host.ps1 -Offline` 成功，EXE/DLL/Flutter 资源匹配 |
| 保留的非 Flutter 路径 | `scripts/build-official-secure-host.ps1 -CheckOnly` 编译成功；原有告警保留，不扩大修复范围 |
| 鸿蒙双 ABI/HAP | `apps/harmony-controller/scripts/build.ps1` 成功，API22 模拟器安装成功；不是 ARM64 真机验收 |

Windows 初次编译发现旧 winapi crate 未导出 `DI_NORMAL`，改用已存在的 Windows crate 常量后通过，未新增依赖。独立 ArkTS 任务探索因 Hvigor 任务名/缓存权限未成功；最终完整 assembleHap 已真正编译 ArkTS 和 NAPI，通过结果以整包日志为准。

本机忽略日志：`flutter-build/input-windows-build-final.log`、`input-windows-tests.log`、`input-integration-analyze.log`、`system-input-hap-build.log`、`system-input-demo-regression.log`。官方 API 依据见 [鸿蒙输入核验](2026-10-03-harmony-system-input-api.md)。

| 产物 | SHA256 |
| --- | --- |
| HAP `entry-default-unsigned.hap` | `FF8EFEF854EB19D596CD74E01E0FB9D017880029D0F8862CD7AFC259DDF09C73` |
| Flutter Debug `rustdesk.exe` | `2B8289894BF4F09C6735E64B34C57A07B0C88337EC0AFFD1472FF2999F4B2D15` |
| 匹配 `librustdesk.dll` | `55DD89A611DD4F96D7935AF6CEA56428B69C1E43B8F0AB802E8A09D883C3480F` |

## 独立审查与回归面

独立审查指出清理失败被普通撤销文案掩盖，已补 CM 粘性失败标记、Flutter 明确提示、核心错误码与鸿蒙说明；限定复核确认关闭，无新增必须修复项。

- `libs/base/protos/message.proto` 仅追加输入协议字段和消息，不改旧编号及密码学；子模块不变。
- controller-core 新增专用输入模块/模式/C ABI，保留原 DEMO 与只读验证；共享的创建/取消/销毁增加新输入状态清理。
- Windows `server.rs` 仅条件模块声明；`secure_host_policy` 与 `secure_host` 增加协商和专用门禁。普通输入服务和队列不变。`secure_video` 仅新控制入口叠加光标。
- CM 新增专用能力/清理失败字段及专用 IPC 回执；Flutter 原模型仅在专用窗口同步这些字段，普通 CM 行为保留。专用批准强制本机点击保护，撤权/结束始终可用。
- Harmony NAPI 只增加有界命令入口，Desk/RemoteSession 增加正式控制模式与暂停；EntryAbility 同步前后台状态，避免 native dispose 后界面保留输入许可。旧 Index 与 DEMO 页面不变。
- 构建脚本增加新测试，正式启动脚本增加默认关闭的输入门禁。旧构建/启动脚本、用户签名、`.clang*` 和原型未纳入改动；16 个保护文件哈希无变化。

## 尚未验收

真实主屏鼠标/拖动/滚轮、物理键与中文确认文本、带按住状态的本机撤权/断开、真实网络断流和多连接互斥仍需用户现场授权后的验证。真机、UAC/安全桌面、公网、无人值守、文件/剪贴板/音频及硬件解码不在本轮通过声明内。软件视频仍受原 1280×720、约 8 fps 限制。
