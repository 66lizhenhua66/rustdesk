# 鸿蒙控制端：原生工作台、ID/中继与可信只读访问

2026-10-07 增量：普通连接现在默认「控制」，连接前可选择「仅查看」。Windows 本次批准、首帧和真实输入启用回执到达后，可直接在远程画面点按、拖动、长按右击及双指滚动；外接鼠标/滚轮/键盘直接映射，键盘按钮调起系统输入法，确认文字自动发送。独立控制面板已移除，收起工具栏不关闭控制。F1—F12、标点、左右 Windows 键与数字小键盘要求同步更新 Windows 被控端。139 项本轮回归和双端构建通过，模拟器安装/启动已验，真实输入与真机仍待验收，见 [本轮记录](../../docs/project/research/2026-10-07-direct-session-input.md)。下面各日期段落保留历史范围。

2026-10-05 增量：工作台支持 IP、设备 ID 自动连接、强制中继与实际路径显示。ID/中继必须填写自建服务、公钥、中继及固定目标身份。新增「登记只读访问」「无人值守查看」「删除本机凭据」；绑定状态和到期时间来自 Asset Store。Windows 必须显式 `-Video -Unattended` 并单独批准登记，普通批准不能登记长期权限。首版无人值守只读、每次最多 15 分钟；授权 30 天，可从 Windows 撤销。详见 [入口说明](../../docs/project/OFFICIAL-SECURE-ENTRY.md)、[协议](../../docs/project/specs/003-trusted-controller-access.md) 和 [本轮验证](../../docs/project/research/2026-10-05-id-relay-unattended-validation.md)。以下 2026-10-04 记录保留前阶段范围。

默认入口为 ArkUI 原生 `pages/Desk.ets`，按照当前原型推荐的 A 方案实现设备工作台、连接等待、远程桌面、会话工具和结束页。签名身份校验、v1 加密、IP 直连、CM 逐连接批准及主屏 VP8 已接入。**普通连接默认控制，可选仅查看；Windows 一次批准连接后自动请求所选输入能力，无第二次本机批准或额外控制面板。新版真实输入验收状态以上方 2026-10-07 记录为准。**

2026-10-04：Windows Debug 整包与鸿蒙双 ABI/HAP 构建通过并已安装，新 HAP 运行于手机模拟器，回环服务已按 `-Video -Input` 启动。84 项 core、9 项旧 DEMO、15 项 Windows、4 项 Flutter 与 10 项鸿蒙模型测试合计 **122 项通过**。正在等待新的本机连接批准，不能宣称 v2 的真实系统输入、中文输入、开关切换或清理已实测通过，详见 [本轮能力切换验证记录](../../docs/project/research/2026-10-04-controller-capability-validation.md)。

实施范围和验收见 [本轮计划](../../docs/project/plans/2026-10-03-native-controller-ui.md)、[本轮验证记录](../../docs/project/research/2026-10-03-native-controller-ui-validation.md)。2026-10-02 已实测真实 Windows 主屏，见 [上轮只读画面记录](../../docs/project/research/2026-10-02-readonly-screen-validation.md)；这份历史证据不代替新界面的本轮现场批准与画面复验。

## 当前界面与能力

- 设备资料：新增、编辑、确认移除、公开配置导入预览、搜索与常用设备；继续读取原有 `controller_profiles/profiles`，保留既有设备。收藏和最近连接使用独立的非敏感存储键。
- 连接：IP 与端口分字段，预存可信设备 ID、公钥和可选指纹；保存不连接。新版 Desk 显式使用 `secure_control` 协商视频与 `input_version=2`。普通连接默认控制、可选仅查看；旧 host 缺少协商能力时失败，不自动降级。身份、批准等待、首帧等待和呈现分别显示真实状态。
- 会话：原生 Surface 保持远端比例，浮动工具栏可收起；选择控制后，在连接获批、支持输入且已有画面时自动请求启用，实际输入等待真实 `input_state` 回执。直接触摸定位、拖动、长按右键、双指滚动及外接键鼠均绑定实际画面，黑边不发送点击。系统输入法只转发确认文字，无单独发送按钮。文件按钮仍禁用。
- 生命周期：返回设备页保留同一会话与 Surface，但立即停止本地输入、释放按下状态并请求关闭键鼠；返回已有会话时可再次选择控制。切回查看也请求关闭，等待真实回执。旋转、触摸/鼠标取消及失焦清理按下状态；收起工具栏/键盘不切换成仅查看。取消、断开、后台和退出清理会话及画面，重连需重新批准。新增输入清理路径的设备实测尚待完成。
- 最近连接：只记录真实获批连接，不预置样例，不保存密码、画面或输入内容。保存设备不代表设备在线或已获访问权。
- 响应式布局：小于 600 vp 为单列和底栏，600–1099 vp 为导航轨和双列，1100 vp 起为完整侧栏与多列设备卡。横竖屏根据当前窗口宽高布局，弹窗及工具内容可滚动。

文件、跨端剪贴板、声音、多屏、硬件解码和画质切换仍未实现。ID/中继和可信设备只读访问已接入，真实服务、公网与设备现场待验；不能把协议 fixtures 当实际网络结果。系统输入法确认文字直接发送，但远端编辑上下文和候选状态没有跨端同步；新版输入法接线仍待设备验证。ArkUI 不使用 WebView，视频像素不经过 ArkTS/JSON。

## 连接与安全边界

正式 Windows 桌面使用现有 Flutter 工程，构建与启动入口为 `scripts/build-windows-host.ps1` 和 `scripts/start-windows-host.ps1 -Video`，见 [正式入口说明](../../docs/project/OFFICIAL-SECURE-ENTRY.md)。视频必须由本机显式 `-Video`（`ORD_SECURE_VIDEO=1`）开启，默认关闭；客户端请求不能开启该门禁，也不能代替 CM 逐连接批准。启动脚本默认只绑定 `127.0.0.1:21120`，没有自动批准、公网监听扩展或系统安全设置修改。旧 `start-official-secure-host.ps1` 仅保留 Sciter 开发壳复测。

键鼠需本机启动 `scripts/start-windows-host.ps1 -Video -Input`（`ORD_SECURE_INPUT=1`）；`-Input` 是默认关闭的实现能力门禁，不是第二次用户批准。Windows 只批准或拒绝本次连接，准入后由控制端选择启用或关闭键鼠，Windows 保留结束连接。输入默认关闭，同一时刻只有一个输入拥有者。请求关联与令牌只保存在 Rust，旧回执不能反转较新的选择，每次启用更新令牌，关闭时清队列；启用期间内部双向心跳以 Windows 3 秒、核心 5 秒为失联阈值。清理失败显式终止并阻止再次启用，需要重启被控进程，不能显示释放成功。

2026-10-03 的 Flutter 界面与只读联调已验证本机三页、窗口缩放、真实批准回执和手机主屏 1280×720，当次接收/呈现为 2037/2032 帧、9816 KiB；主动断开清屏、重连重新审批、待批准关窗均通过。之后已补验拒绝按钮与活动视频中直接关窗，手机均清屏、XComponent 数为 0。等待批准时会创建待显示组件，不能把组件存在当成已显示视频。详见 [前轮 Flutter 界面记录](../../docs/project/research/2026-10-03-windows-host-ui-validation.md)；这些只读结果不替代新版系统输入、中文或撤权实测。

Windows 的“批准连接”只确认连接获批，CM 回执不表示视频已协商或首帧已送达；上述画面结果来自鸿蒙原生 Surface 的实际观察。下文的 82 项回归和真实 Windows 画面记录属于此前鸿蒙界面＋开发壳组合，本轮 Flutter 验收以上述独立记录为准。

正式表单默认端口 21120；IP 地址支持标准 literal IPv4/IPv6，端口和资料合法性由共享 Rust 核心验证。旧诊断页未填端口时仍采用原有默认值 21118。公钥和指纹是公开信任材料，必须从可信来源核对；不能从网络预检结果自动建立或替换信任。普通上游明文 IP 入口缺少所需签名握手，会被拒绝，不会降级继续。

视频使用 VP8 软件编解码，单主屏不超过 1280×720、约 8 fps 上限，相同像素不重复发送。仅新 control 路径由 Windows GDI 叠加真实光标，鸿蒙不绘制预测光标。获批前不启动采集；连接批准不代表已呈现首帧或已允许键鼠。

## 保留的连接诊断

在偏好设置中打开连接诊断，进入原有 `pages/Index.ets`。其设备 CRUD、IP/设备 ID/中继资料保存及公开配置导入路径保留；该旧诊断页的 ID/中继仍仅保存资料，实际连接使用新工作台。

| 入口 | 实际行为与边界 |
| --- | --- |
| 网络预检 | 只检查指定 IP 的网络与协议首帧，最多等待 2 秒，不发送应用载荷，不验证身份、不授予权限 |
| 验证加密登录 | 核对预存 ID/公钥、签名与 v1 加密后执行单次登录，确认即关闭；显示登录已确认也不代表可以操作；2FA 继续流程未接入 |
| 连接正式入口 | 正式 Windows 安全入口的持续非媒体诊断，需本机批准，仅验证连接，不启动画面或系统输入 |
| 查看桌面（只读） | 旧页面保留的正式视频入口，同样受视频门禁、身份/加密和 CM 批准约束 |
| 连接演示端 | 独立 Windows DEMO 通道，核对配对码、现场批准、独立输入授权；光标与文本仅作用于演示窗口，不是系统桌面控制 |

诊断页本次登录密码不写入 Preferences，在启动、取消、切换设备或离开页面时清空；JavaScript 字符串不保证立即物理擦除。单次登录 UI 默认总时限 10 秒，API 上限 30 秒；持续 DEMO 握手上限 60 秒、会话最长 30 分钟，独立 Windows DEMO 的现场批准另有 30 秒时限。取消关闭已连接 socket；进行中的系统 connect 最迟在 2 秒上限结束。每个环境最多允许 4 个尚未退出的网络 worker，旧回调在取消或退出后丢弃。

独立演示端的运行步骤见 [DEMO 指南](../../docs/project/DEMO.md)。DEMO 的独立输入授权及演示窗口行为保持原样，不能作为正式系统键鼠控制的验收证据；正式输入使用上述一次连接批准、控制端开关、Windows 执行点校验与断开清理流程。

## 结构

- 会话输入已接入跨平台共享核心：手势、坐标、滚轮、按键状态与确认文本分块由 `libs/controller-core/src/interaction.rs` 实现，鸿蒙 NAPI 和 Flutter FFI 复用同一份。`entry/src/main/ets/input/` 只保留鸿蒙事件转换及会话 UI 状态适配。扩展操作体验的位置与约束见 [输入模块说明](entry/src/main/ets/input/README.md)。
- 触屏画布使用共享 `controller_canvas_*_v1`：默认适应窗口，支持 1.0x—10.0x 本地缩放、旋转保留画布中心、单指平移、1 秒长按拖动准备、双指右键/滚轮/捏合与一键还原。工具中可选触屏/指针模式；只读也可本地缩放。窗口避让矩形与实际视口求交后叠加工具栏和 24vp 黑边，避免重复扣安全区。Flutter 可复用画布已接相同 ABI，真机呈现和操作由用户后续验收，见 [交付记录](../../docs/project/research/2026-10-08-canvas-completion.md)。
- `entry/src/main/ets/pages/Desk.ets`：正式工作台、设备存储及会话调度；`components/RemoteSession.ets` 承载原生画面与工具栏，`model/DeskModels.ts` 将核心事件投影为界面状态。
- `entry/src/main/ets/pages/Index.ets`：保留旧连接诊断和独立 DEMO 操作路径。
- `entry/src/main/cpp`：薄 NAPI 桥接、原生 libvpx 解码和 NativeWindow 渲染。网络操作在后台，取消不在 UI 线程等待网络 worker 退出。
- [libs/controller-core](../../libs/controller-core/README.md)：共享配置验证、认证、协议和会话核心，通过版本化 C ABI 供平台适配调用。
- `scripts/build.ps1`：构建 x86_64/ARM64 Rust、libvpx、C++ 库及 HAP。Android/iOS 后续复用核心，不复制鸿蒙界面或协议实现。

## 构建与验证

从 RustDesk 仓库根目录运行：

```powershell
node --experimental-strip-types --test apps/harmony-controller/tests/desk-models.test.ts apps/harmony-controller/tests/input-models.test.ts apps/harmony-controller/tests/access-models.test.ts apps/harmony-controller/tests/session-input.test.ts
cargo test --manifest-path libs/controller-core/Cargo.toml --locked
cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked
& ./scripts/build-windows-host.ps1 -Test
& ./apps/harmony-controller/scripts/build.ps1 -DevEcoRoot 'G:\Huawei\DevEco Studio'
```

使用 Rust 1.96.1、DevEco 内置 SDK 6.1.1.125/API24，HAP 兼容 API22；需已安装 `x86_64-unknown-linux-ohos` 和 `aarch64-unknown-linux-ohos` 标准库。开发包为 `entry/build/default/outputs/default/entry-default-unsigned.hap`。构建缓存、截图、布局、日志及签名材料不进 Git；不要覆盖用户本地 `build-profile.json5`。

## 历史界面验收（2026-10-03）

此前 77 项回归与 5 项界面模型测试共 **82 项通过**，双 ABI/HAP 构建通过。API22 手机模拟器已验证设备 CRUD、冷启动读取、横竖屏切换、批准等待及超时；使用合成 VP8 视频验证了浮动工具栏、工具面板、收起/展开、会话旋转、返回设备保持同一 Surface 和断开清屏。合成视频没有采集真实 Windows 桌面，不替代正式 CM 批准。API24 2in1 模拟器已验证中宽/宽屏工作台，窗口宽度变化不等于平板真机验收。

本轮 Windows Debug 首次因运行中的 `target/debug/rustdesk.exe` 占用而失败（更新产物时访问被拒绝）；核验没有活动连接后，仅停止本仓库对应的测试进程，重试构建成功，随后恢复 `127.0.0.1:21120` 回环视频服务与 CM，未自动批准。首次失败与最终成功分别记录在 `artifacts/ui-windows-debug.log`、`artifacts/ui-windows-debug-final.log`。

**本轮新界面的真实桌面复验已通过。** 用户在 Windows CM 实际点击普通蓝色“接受”后，API22 手机模拟器的新 Desk 页面显示真实 Windows 主屏 1152×720。初次观察接收/呈现为 335/336 帧、1650 KiB，最终为 1327/1328 帧、6671 KiB；接收和呈现计数独立限频更新，不要求同次读取完全相等，也不作为吞吐或延迟基准。

真实会话已实测横竖屏切换、返回工作台保留同一 Surface 和活动目标、返回会话、断开清屏（Surface 数为 0）、重连重新等待本机批准，以及取消新等待请求后 Surface 数仍为 0。验收收尾时控制端已回设备页，正式服务只在 `127.0.0.1:21120` 监听，没有活动连接；未自动批准或复用上次授权。

真机验证继续后移；上述历史界面联调未开放系统键鼠。当前新增输入的实际验收状态以上文 2026-10-04 记录为准；ARM64 编译、模拟器和本机回环联调不代表 HarmonyOS 6.1 真机或公网验收通过。
