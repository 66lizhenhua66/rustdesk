# 鸿蒙优先控制端的代码边界与跨平台适配调研

- 调研日期：2026-09-30。
- 产品源码基线：`a7f2260203befb7e9c70b585219f0f0b5ca57703`。
- 调研时工作分支：`feat/harmony-controller`；HEAD 为 `15679d88ab602ea490dad6e0f48ead72e53b1a6d`。基线之后的提交为项目文档更新，本报告涉及的源码路径与基线之间没有差异。
- 方法：先使用现有 GitNexus 索引查询调用关系和符号，再核对工作区源码。未进行编译、鸿蒙真机运行或网络性能测试。
- 目标：鸿蒙 6.1 手机、平板控制端优先，未来 Android、iOS 能复用协议与会话核心；Windows 被控端沿用 RustDesk 基础。

## 1. 结论与边界

建议采用 **ArkTS/ArkUI 鸿蒙界面 + C++ NAPI 适配 + 新增版本化 C ABI + RustDesk 会话与协议实现 + 独立平台媒体适配**。后续 Android、iOS 可以采用 Flutter 界面或各自原生界面，通过适配层继续复用核心。共享目标是协议、认证、会话和部分业务，不是让 ArkUI 页面直接运行在其他平台。

RustDesk 已提供动态库、静态库构建形式，但当前源码不是保证可直接嵌入鸿蒙的独立 SDK。UI、原生平台能力、会话、媒体、控制与被控角色仍有交叉依赖。首期在当前 fork 内增量增加桥接和平台模块，保留既有 Flutter/桌面运行路径；不先拆仓库，不为预想中的多协议重构全部旧代码。

本报告中的鸿蒙 ABI、模块和状态名称是设计建议，均不代表当前仓库已经实现。鸿蒙 SDK API、Rust target、三方依赖和编解码能力需要在现有 6.1 真机环境验证。

## 2. 已核实的代码证据

| 编号 | 源码位置及符号 | 当前事实 | 设计含义 |
| --- | --- | --- | --- |
| C-01 | [src/client.rs](../../../src/client.rs)：`Client::start`，约 362 行；`Interface`，约 4875 行 | `Client::start` 接收 `impl Interface`，接口提供登录、消息、配置和连接错误处理。 | 复用官方连接、认证和加密流程；不在 ArkTS 重写打洞或密码学协议。 |
| C-02 | [src/ui_session_interface.rs](../../../src/ui_session_interface.rs)：`Session<T: InvokeUiSession>`，约 61 行；其 `Interface` 实现，约 1778 行 | 会话保存登录配置、命令 sender、工作线程、权限状态和连接轮次；UI handler 是泛型参数。 | 新增鸿蒙 UI handler 是可评估的接入点，可复用会话状态与命令路径。 |
| C-03 | [src/ui_session_interface.rs](../../../src/ui_session_interface.rs)：`InvokeUiSession`，约 1693 行 | 回调包含画面、光标、权限、质量、安全状态和指纹，也包含文件、语音、录屏、打印、终端等能力。 | 现有接口较宽，不是精简 SDK；新的对外 ABI 应只暴露首期能力，未实现功能明确不支持。 |
| C-04 | [src/flutter_ffi.rs](../../../src/flutter_ffi.rs)：`EventToUI`，约 96 行；`session_add_sync`，约 148 行；`session_start`，约 189 行 | 使用 `flutter_rust_bridge::StreamSink`、`SyncReturn` 和 Flutter 会话注册表；事件包含字符串、RGBA、纹理和光标。 | 借鉴命令与事件语义，新增独立桥接，不让鸿蒙依赖 Flutter FFI，也不直接复制整套 Flutter 状态管理。 |
| C-05 | [Cargo.toml](../../../Cargo.toml)：`[lib]`，约 11 行；平台依赖，约 93、98 行；[src/lib.rs](../../../src/lib.rs)：约 6、11、26、34 行 | 支持 `cdylib/staticlib/rlib`；多处把 `not(android/ios)` 当桌面平台，从而引入 Sciter、enigo、clipboard、PTY 等；模块 gate 混合 UI 和运行角色。 | 新 target 不能直接当 Linux/Android；需要逐项处理编译条件与依赖，控制端构建不应意外启用被控服务或桌面依赖。 |
| C-06 | [src/platform/mod.rs](../../../src/platform/mod.rs)；[libs/scrap/src/lib.rs](../../../libs/scrap/src/lib.rs) | 存在 Windows/macOS/Linux、Android 等分支；本次检查的源码和 Cargo 范围未找到 Harmony/OHOS 平台分支。 | 鸿蒙工具链、系统服务和原生库适配仍待实施，不能仅因 Rust 支持交叉编译就声明可用。 |
| C-07 | [src/client/io_loop.rs](../../../src/client/io_loop.rs)：`Remote::new_video_thread`，约 2655 行；[src/client.rs](../../../src/client.rs)：`start_video_thread`，约 4003 行 | 按显示器创建视频处理路径，编码帧队列与媒体消息分开；解码后调用 `on_rgba`，纹理回调有 `vram + flutter` 条件。 | 已有媒体线程边界可复用；需要新增鸿蒙渲染适配，当前纹理路径不等同于鸿蒙 Surface 支持。 |
| C-08 | [src/client.rs](../../../src/client.rs)：`VideoHandler::new/handle_frame`，约 2685、2708 行；[libs/scrap/src/common/codec.rs](../../../libs/scrap/src/common/codec.rs)：`Decoder::handle_video_frame`，约 631 行 | 支持 VP8/VP9/AV1 分支；H.264/H.265 与 `hwcodec/vram/mediacodec` 等 feature 关联；视频线程处理解码失败、能力更新和刷新。 | 解码格式需要真实协商；先跑通一种支持的解码组合，再做鸿蒙硬解，不默认 H.265 或任何硬解后端可用。 |
| C-09 | [src/ui_session_interface.rs](../../../src/ui_session_interface.rs)：约 1324 行的重连线程、`close` 约 1417 行、`io_loop` 约 1956 行；[desktop_render_texture.dart](../../../flutter/lib/models/desktop_render_texture.dart)：约 42、98 行 | 现有线程入口拥有 Tokio runtime；`close` 投递 `Data::Close`；Flutter 纹理销毁先注销 native 指针，再销毁资源。 | 不能在已有 Tokio runtime 内嵌套调用创建 runtime 的入口；关闭命令提交成功不等于工作线程、帧和 native 资源已释放。 |
| C-10 | [src/ui_session_interface.rs](../../../src/ui_session_interface.rs)：约 1965 行的 RDP 分支；[src/port_forward.rs](../../../src/port_forward.rs)：`run_rdp/listen`，约 19、87 行 | RDP 路径建立端口转发，Windows 路径启动 `mstsc`；不是内置跨平台 FreeRDP 会话与画面引擎。 | 不能把现有 `ConnType::RDP/is_rdp` 当成已实现的多协议控制器；未来 RDP/VNC/Moonlight 仍需各自适配。 |

## 3. 分层与首期职责

建议逻辑分层如下，具体目录名在实现任务中确定，不把这些名称理解为现存模块：

```text
鸿蒙 ArkTS/ArkUI
  设备配置、授权提示、画面工具栏、触控/IME、应用生命周期
              |
鸿蒙 C++ NAPI 适配器
  参数校验、异步命令、低频事件、Surface 绑定
              |
版本化 C ABI / bridge facade
  不透明句柄、命令、状态、能力、错误、资源所有权
              |
RustDesk 会话与协议实现
  连接、认证、消息、安全状态、输入命令、画质协商
              |
平台适配
  解码/渲染、音频、凭据存储、剪贴板、网络/前后台
```

首期优先新增鸿蒙专属 UI handler 与桥接模块，在共享入口增加必要的薄调用或平台 gate。先评估 `Session<T>` 能否以合理成本在鸿蒙编译，不预先承诺其所有依赖都可直接移植。处理旧 trait 未支持回调时要明确声明能力不可用；不得通过空函数让 UI 误认为操作成功。

对外 ABI 的最小功能范围：

- 初始化、版本与 ABI 能力查询、非敏感服务器配置和凭据引用。
- 创建、连接、取消、重连、断开和销毁会话；提交密码或其他认证响应。
- 查询对端信息、安全状态、协议能力和已授予权限。
- 视频 Surface 绑定/解绑、显示器选择、画质/帧率请求。
- 指针、鼠标按钮、键盘、文本输入。
- 订阅结构化事件、获取连接与媒体统计、明确释放资源。

文件传输、剪贴板、终端、摄像头、隧道和远程提权不因底层已有实现就自动进入新客户端。首期未实现的功能不导出，后续按单独需求开放并进行权限验收。

## 4. 状态、能力与安全信息

建议对 UI 暴露可以区别网络连通与授权完成的状态：

```text
Idle → Connecting → VerifyingPeer → Authenticating
     → AwaitingApproval → Active → Closing → Closed
任一阶段可以进入 Error；取消、失败和关闭都要释放资源。
```

实际认证与批准顺序以协议路径为准，上图是产品可观察阶段，不要求为其重写协议。状态事件还应携带会话 ID、连接轮次或 generation、请求 ID、错误码和必要的可展示信息。

`InvokeUiSession::set_connection_type` 和 `set_fingerprint` 提供 UI 可消费的安全信息，但展示一个锁图标不构成权限校验。新客户端不得将 transport connected、拿到画面或 UI 本地选项解释为授权依据。缺少预期安全能力、指纹不符时按产品策略拒绝，不默认继续不安全连接。

区分三件事：

1. 本地控制端已实现什么。
2. 对端协议和设备支持什么。
3. 该会话被授予什么操作权限。

可用功能取三者交集。权限变更由对端事件更新 UI，但最终每类操作仍由被控端执行授权检查，不能依赖控制端隐藏按钮。

## 5. 媒体路径与内存生命周期

### 5.1 画面不经过 ArkTS 逐帧序列化

先复用可运行的 Rust 解码路径取得正确图像，再实现或接入鸿蒙原生解码/渲染。ArkTS 接收的是尺寸、统计、错误、状态等低频事件，不以 JSON/Base64 传递整帧像素。

硬解优先级不等于硬解必然可用。需验证 codec、profile、分辨率、像素格式和设备支持；解码器重建、Surface 重建、网络切换和关键帧恢复都属于同一生命周期。支持能力必须与实际已验证后端一致，失败时按已实现方案切换解码格式或干净失败。

### 5.2 ABI 资源规则

- 不直接把 Rust `Vec`、`String`、trait 对象或泛型布局暴露给其他语言；使用固定宽度字段、长度、版本化结构和不透明句柄。
- 每个 buffer 明确包含格式、宽高、stride、timestamp、frame/session/display 标识以及释放规则。
- 临时借用像素指针不得在回调返回后继续使用；异步消费者必须拿到 owned buffer、引用计数资源或有明确 retain/release 的平台句柄。
- NAPI 的线程安全回调只负责调度事件，不把非线程安全 ArkTS 对象交给媒体或网络线程。
- ABI 边界校验句柄、长度、枚举、尺寸和状态；返回结构化错误，不让 Rust panic 或 C++ 异常穿过 FFI。
- Surface 解绑顺序为停止新写入、注销回调、确认在途操作完成、释放媒体引用、销毁 Surface。不能以固定延迟替代明确所有权和关闭完成信号。
- 关闭命令应是幂等的。`Closing` 后不接受新的普通输入；最终关闭完成与资源可销毁事件分开于“命令已入队”。旧 generation 的帧或回调不得更新新会话。

### 5.3 线程与流控

ArkUI 线程只做 UI，桥接快速入队；网络与会话逻辑保留 Rust 工作线程，媒体使用专用线程。保持现有 runtime 的拥有关系，不在 Tokio async 回调中嵌套调用创建 runtime 的入口，也不在 UI 线程阻塞等网络/解码结束。

新桥接队列要有上限。鼠标移动可以合并，但按键 down/up、按钮释放、撤销权限和关闭不能随意丢弃；队列压力下应明确拒绝或取消输入并恢复按键状态。编码帧有帧间依赖，不能简单“只保留最后一个编码包”；丢帧恢复要遵循原解码链路和关键帧刷新机制。解码后的呈现帧可以按既定策略丢弃过时画面以控制延迟。

## 6. 鸿蒙与未来平台适配

| 能力 | 鸿蒙首期方向 | 后续 Android/iOS |
| --- | --- | --- |
| 界面 | ArkTS/ArkUI，手机和平板布局 | 可采用 Flutter 统一两端界面，或原生 UI；不复用 ArkUI 页面 |
| 协议与会话 | RustDesk 核心经独立 C ABI 接入 | 继续调用同一核心，不复制 RustDesk 源码各自维护 |
| 媒体 | 实测鸿蒙解码 API 与原生 Surface；保留可行回退 | 各平台实现媒体后端，不能假设鸿蒙解码器直接通用 |
| 绑定 | C++ NAPI、线程安全事件桥接 | JNI、C/Swift 或 Flutter FFI，保持核心不含平台 UI 对象 |
| 安全存储 | 使用 6.1 SDK 验证可用的 HUKS/凭据存储能力 | Keychain/Keystore 或对应安全存储实现 |
| 系统交互 | IME、外接键鼠、音频、前后台、网络变化 | 各平台适配；不能依赖一个平台的键码与生命周期 |

普通配置仅保存非敏感参数。密码、设备私钥和长期令牌通过安全存储接口使用，不直接存普通 preferences 或写入日志。系统安全存储接口及后台运行能力仍需真机核实，不承诺应用切后台后永远维持连接。

共享层避免 ArkTS、JNI、Swift 和 Flutter 专属类型；平台回调、buffer 和 surface 以明确契约实现隔离。未来 Flutter Android/iOS 接入可复用会话/协议核心，但仍需处理插件、签名、系统权限与媒体后端。iOS 阶段需要相应 macOS/Xcode 环境和签名条件。

## 7. 多协议扩展的合理范围

首期只实现 RustDesk，通过少量明确字段预留 `ProtocolId`、连接目标、能力集合和通用输入/显示/质量语义。RustDesk 的 peer ID、打洞/中继配置和 codec 选项保留在专用 options 内，不能将其强加给所有协议。

后续接入 RDP、VNC、Moonlight 时增加对应适配器和依赖，验证真实差异后再扩展公共接口。现有 RDP 隧道不等同于内置 RDP 客户端；SSH 是终端能力，不强行套入视频会话。控制端支持多个协议不要求自有 Windows 被控端也实现多个服务端协议。

不为通用接口重写既有 Flutter/桌面路径。一个新增协议或平台的实现尽量留在其专属模块，避免仅为新功能让所有旧调用方增加占位参数或承担新的状态机。

## 8. 建议验证顺序

| 阶段 | 内容 | 验收证据 |
| --- | --- | --- |
| H0：构建边界 | 6.1 真机加载 native library；版本查询、结构化错误；初始化/销毁；控制端专用 target/feature 和依赖清单 | 可复现构建步骤、固定依赖与 target、真机运行记录，不意外启动被控角色 |
| H1：最小会话 | 连接官方 Windows 被控端；认证/现场批准、单显示器、一种可用解码、基本键鼠；分开验证自建中继和 IP 直连 | 有画面和输入结果；拒绝认证/取消连接可关闭；不存在由 UI 伪造授权的路径 |
| H2：鸿蒙体验 | 硬解探测与失败处理；触控板/直接触控、中文 IME、外接键鼠；旋转、切网络、后台/前台、重连 | 记录能力矩阵和失败行为；坐标一致、无卡键、无销毁后回调、资源释放可观察 |
| H3：跨平台边界 | 用最小 Android/iOS 适配验证或平台 stub 检查核心依赖，逐步开始未来端构建 | 核心不含鸿蒙 UI 对象，新增端能调用版本/会话接口；stub 不能代替最终真机验证 |
| H4：增量功能 | 音频、按权限开放的剪贴板、多显示器；以后再加多协议 | 各功能具备权限拒绝、异常断开和资源回收验收 |

AUTO 打洞、IPv6、严格 NAT、移动网络切换必须单独记录结果；复用官方源码不等于这些路径已在鸿蒙验收。媒体性能、省流、延迟和电量目标应在可运行基线后测量，不以架构设计推断实际指标。

## 9. 回归范围与未验证事项

本次只新增本报告，不改变产品代码和运行行为。上述设计实施时，预期触点包括新的鸿蒙平台/桥接模块、必要 Cargo/build gate、`src/lib.rs` 的模块声明，以及少量会话或媒体适配入口。现有 Flutter、Windows/macOS 控制/被控路径在新功能关闭时应保持原行为。

尚未证明：鸿蒙 toolchain 和全部 native 依赖可编译、所有官方传输路径可用、指定 codec 可硬解、后台连接可持续、平台安全存储接口满足全部需求。应以 H0–H2 的实际证据决定后续拆分和投入，不把本报告当作这些能力已经完成的声明。
