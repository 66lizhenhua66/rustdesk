# 正式主屏键鼠输入实施计划

日期：2026-10-03。基线 `286cd2457`，分支 `feat/harmony-controller`。用户已授权按“界面验收收口 → 独立授权的真实键鼠”顺序直接实施；保留用户原型、签名、`.clang*` 和旧脚本本地内容。

## 目标与选择

在现有 Flutter Windows 被控工作台和 ArkUI 鸿蒙控制端接入主屏键鼠。查看批准不授予输入；Windows 本地 `ORD_SECURE_INPUT=1` 显式启动门禁与逐连接“允许键鼠”缺一不可，默认仍只读。真机和公网继续不作为本轮通过声明。

采用严格入口专用、同步执行的 Windows SendInput adapter。旧 `Connection::input_mouse/input_key` 在严格分支没有消费者，且普通输入服务有无界队列、portable service 转发及桌面切换，不满足本轮撤权要求，故不复用这条执行链。也不把系统输入塞入 DEMO 的 800×450 指针接口。共享协议继续由 Rust 维护。

第一片覆盖：主屏移动、左右/中键按下释放、点击/拖动、滚轮、常用物理按键、显式提交的 Unicode 文本。手机使用触控板与明确鼠标/滚动/键盘控件；未开放文件、剪贴板、终端、提权、无人值守、UAC/安全桌面。文本只发送用户确认后的内容，不发送输入法预编辑文本。

## 协议与授权

- 保留 `secure_video` 的四字段只读协议和旧 C ABI。新增显式 `expectedPeer=secure_control`，LoginRequest 字段 18 为 `ord_input_version=1`；新版 Desk 使用该可协商模式，诊断页原只读路径保留。旧 host 无此字段会拒绝，不能静默降级成输入会话。
- 在本仓库 `libs/base/protos/message.proto` 的 Message union 34/35 增加 `OrdInputState` / `OrdInputEvent`；不修改子模块。State 字段为 `version=1,supported=2,enabled=3,grant_token=4`。Event 带版本、16 字节授权令牌及受限输入命令。初始/撤权状态令牌为空，只有本机独立授权后产生随机令牌。
- 每次授权生成全新随机 16 字节令牌；Windows 在真正执行 SendInput 前同时验证令牌、当前连接许可、主屏/交互 Default 桌面和未锁屏。还须遵守既有本机 keyboard 权限上限。旧授权中的排队或在途事件不能在重授后执行。
- 客户端有界输入队列，连续移动可合并；入队与发送前核验当前令牌。撤权、取消和断开使令牌失效并清队列。Windows 无另一个待执行输入队列，CM 事件优先处理。
- 授权期间核心每秒发送内部 `keep_alive`（OrdInputEvent command 9，携当前令牌，不暴露到 ArkUI）；3 秒未收到有效令牌事件/心跳，Windows 下一个秒级检查即撤权释放。Windows 回显有效心跳，核心在已获输入许可时 5 秒无有效入站也会停止会话；每次控制写入限时 2 秒。TCP 半断开没有 EOF 时也不能让按住状态持续到会话总超时。
- Windows 使用单一输入拥有者租约，同一时刻仅一个连接可控制；其他连接仍可查看。按连接记录成功注入的按键/鼠标按下状态，撤权、关窗、断开、错误和 Drop 均释放。不会持久保存授权或复用上一连接许可。
- 正常注入与清理释放都更新时间戳/输入标记，延续 CM 的远端点击保护，防止远端事件代点现场批准。SendInput 返回数量必须核对；释放不受已撤销许可的阻拦，释放失败须可见并阻止新授权，不能宣称已全部释放。
- 点击保护只用于增加权限；撤权与结束连接始终可用，不能让持续远端输入刷新时间戳而阻止本机停止操作。
- Flutter 复用 `cmSwitchPermission` 命令入口，但专用模式只接受 `keyboard`，权限与支持能力由服务回执更新，不能乐观置为启用。CM 客户端快照增加专用支持状态，普通模式保留原行为。

## 文件与任务

### 1. 收口现有界面

- [x] 实测活动视频中关闭 Flutter 窗口：手机结束、XComponent 0、CM 进程退出。
- [x] 重新实测真实拒绝：请求在 UTC 15:05:04 发起，15:05:46 已结束，Flutter 记录“已提交拒绝请求”；手机通用连接失败、XComponent 0。此前控件索引失效和自然超时不计为拒绝通过。
- [x] 补充既有验证报告及进度，保留拒绝的手机文案目前不区分原因这一限制。

### 2. 共享核心与 C ABI

负责人：core 子任务。文件：`libs/base/protos/message.proto`、`libs/controller-core/src/session.rs`、新增 `libs/controller-core/src/input.rs`（仅本切片状态/命令校验），`libs/controller-core/include/session.h`、对应现有测试文件。

- [x] 增加协议和 opt-in 模式；新 PeerInfo 严格核验 VP8/单屏/input v1，旧只读路径不接受权限提升。
- [x] 新 `controller_session_send_input_v1(session, json)`；命令明确为 move、button、wheel、key、text、release_all，拒绝未知字段、非法坐标/键/过长文本。绝对坐标为 0..65535；触控板使用 move_relative（command 10），各轴为 -65535..65535 的归一化增量，由 Windows 读取真实光标后移动并限制主屏。滚轮以一个 Windows WHEEL_DELTA 为一格，单次 dx/dy 限制为 -10..10。
- [x] 验证未授权不能入队、撤权清队列、重授旧令牌不生效、错误/未知状态不能启用输入、旧 DEMO 与 secure_video 行为保留。

### 3. Windows 执行与授权回执

负责人：host 子任务。文件：新增 `src/server/secure_input.rs`，`src/server/connection/secure_host.rs`、`src/server/secure_host_policy.rs`、`src/server/secure_video.rs`、`src/ui_cm_interface.rs`、必要条件模块声明。

- [x] 本地门禁、独立授予/撤销、唯一拥有者、执行点校验、受限 SendInput 和释放全部自有按键/按钮。
- [x] 鼠标主屏映射与输入桌面检查；不调用提权、portable service、SAS、锁屏或切换桌面接口。SendInput 失败则撤权/清理，不能假装成功。
- [x] 新控制视频路径显示来自 Windows 的真实光标，按采集缩放处理光标与热点；不把手机预测位置当作 Windows 反馈。
- [x] 用可控注入器验证门禁/撤权/断开释放、旧令牌和非法命令；自动化测试不操作用户真实桌面。

### 4. 两端界面与平台桥接

负责人：root（Flutter）、Harmony 子任务（NAPI/ArkUI）。文件：Flutter `secure_host_page.dart`、`secure_host_workspace.dart`、`server_model.dart`；鸿蒙 `RemoteSession.ets`、`Desk.ets`、会话服务/模型、NAPI 与 native 类型声明。

- [x] Flutter 只对已批准且支持的连接显示独立“允许键鼠/撤销键鼠”；按钮等待真实回执，始终可结束连接。
- [x] 鸿蒙正式 Surface 保持原路径；低频输入状态与命令经 NAPI/C ABI，像素不进入 ArkTS/JSON。
- [x] 仅真实输入授权后启用触控板/按钮/键盘。中文用显式确认文本；键盘与文本通路不重复发送。
- [x] 返回工作台、隐藏会话、旋转、触摸取消、离开前台时释放按键和指针状态；Desk 保留 Surface 的路径需显式处理输入可见性，不能只依赖 aboutToDisappear。
- [x] 查询项目华为知识 MCP 的 TouchEvent、AxisEvent、TextInput/IME、焦点及生命周期原文，按 API22 兼容线和 API24 目标构建。

### 5. 验证与交付

- [x] 核心/Windows 授权和释放回归、Flutter widget 与定向 analyze；Windows Debug 整包和鸿蒙双 ABI/HAP 构建。
- [ ] 手机模拟器真实只读连接、用户本机独立授权后在专用测试编辑窗口验证鼠标/按键/中文；本机撤权后停止输入，断开和重连不保留授权。不让真实测试操作系统设置或外部应用数据。
- [x] 独立安全/最小回归面审查，更新进度和验证记录，检查保护文件哈希；实现与构建结果保存本地提交，不推送，刷新已有 GitNexus。现场输入仍单列等待本机授权。

## 验收判据

本机未显式启用、查看未批准、输入未独立授权、已撤权、旧授权令牌、会话已断开和安全桌面条件下，真实输入均不执行；撤权后的释放先于 UI 宣称关闭。普通 UI、旧只读/DEMO 通道保持现有行为。拒绝、失败与未实现能力均有真实状态，不用本地动画代替执行结果。
