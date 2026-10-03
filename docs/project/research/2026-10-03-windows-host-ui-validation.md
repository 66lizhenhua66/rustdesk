# Windows Flutter 被控端原型界面验证

日期：2026-10-03。基线 `87a0aa459`。对应 [Flutter 实施计划](../plans/2026-10-03-windows-host-ui.md)。本轮正式界面限定现有 Flutter Windows 工程，Sciter 开发壳不作为交付。

## 当前进度

此前本轮未提交的 Sciter UI 扩展已撤回。Windows 正式页面已在现有 Flutter 工程实现；复用 ServerModel/FRB 与严格服务，仅新增专用 CM 路由、能力查询和批准回执。鸿蒙页面、用户原型/签名/工具配置保持原样。

Debug 整包构建和本轮十项定向测试已通过。用户完成 Flutter 工作台现场批准后，已验收手机模拟器的真实 Windows 主屏、Windows 主动结束、清屏、重连重新等待批准，以及待批准时关闭工作台的连接清理。原始日志与截图留在忽略的 artifacts，含真实桌面的材料不进入 Git。

## 工具链与依赖

使用仓库 Windows x64 CI 对应的 Flutter 3.24.5 / Dart 3.5.4、flutter_rust_bridge_codegen 1.80.1、cargo-expand 1.0.95、Visual Studio Build Tools 2022。隔离 SDK 位于工作空间 `.tools/flutter-3.24.5/flutter`，没有覆盖用户全局 Flutter。SDK 归档取自 Flutter 中国镜像，SHA256 为 `b8a7485acd3c6fb23a76b7ac09f89e8d93d62fbff7147c6f5f8c5686d949eeac`。

九项 Git 依赖保持原锁定提交，归档校验匹配；根 Cargo.lock 和直接业务依赖声明不变。`pubspec.lock` 仅补齐已声明 flutter_test 的 SDK 配套测试依赖，以及原缺失的五项传递依赖，vector_math 版本不变。`flutter pub get --offline --enforce-lockfile` 通过。详细来源和校验记录保存在忽略目录 `apps/harmony-controller/artifacts/flutter-build/PROVENANCE.md`。

SDK 初始深路径触发 Windows 文件枚举失败，将隔离 SDK 移到工作空间短路径后重新生成 package_config 解决；没有修改系统长路径或安全设置。桥接首次离线 metadata 缺失的 registry 包已按 Cargo.lock 校验补齐。

## 本轮验证进度

| 范围 | 实际结果 |
| --- | --- |
| 纯 Flutter 工作台 | 最新三组 widget 测试通过；覆盖按连接 ID 批准/结束、确认中状态、禁用能力、批准不代表视频，以及 1120×760、840×620、840×420 布局 |
| 新页面定向 analyze | mode/page/workspace 零问题；同时检查 main/server_model 时只有 main 原有的六条弃用提示，无错误 |
| 专用 CM 参数选择 | Flutter + ord-secure-host 下两项单元测试通过 |
| 严格审批与视频 | Flutter + ord-secure-host 下批准门禁一项、视频四项通过，含显式开关、尺寸、真实 VP8 编解码和满队列停止 |
| 不带 Flutter 的严格服务检查 | cargo check 通过；保留旧开发壳路径 |
| Flutter + ord-secure-host Rust DLL | Debug 编译通过，现有告警未扩大修复范围 |
| Windows Flutter 整包 | `scripts/build-windows-host.ps1 -Offline` 最终退出 0，生成 Flutter EXE、匹配 Rust DLL 与运行资源 |
| 实际 Flutter 桌面 | 三页切换、无请求常驻、最大化/还原及真实 pending 请求均已观察；截图与原型核对侧栏、绿灰配色、双栏卡片和固定操作栏 |
| 手机联调 | 首次未批准请求按规则超时；后续用户亲自批准，真实画面、Windows 主动断开清屏和重连重新审批均通过，详见下节 |

桥接首次缺 `stdbool.h`，旧 ffigen 虽报告 fatal 却返回 0，错误生成遮蔽 Dart bool 的 typedef。显式传入 `--llvm-compiler-opts` 的 Windows target 和独立 `-resource-dir "路径"` 参数后，重新生成无 SEVERE/fatal，生成文件 analyze 零问题。脚本保留诊断检查，拒绝这种错误桥接。带空格路径不可用 `-resource-dir="路径"`，旧 ffigen 分词器会拆错。

最终日志：`flutter-build/windows-build-final.log`、`windows-tests-final.log`、`integration-analyze-final.log`、`frb-generate-final.log`。截图：`flutter-host-idle.png`、`flutter-host-records.png`、`flutter-host-settings.png`、`flutter-host-pending.png`、`flutter-host-timeout-ended.png`。

Debug 整包目录：`flutter/build/windows/x64/runner/Debug/`。本次 EXE SHA256 为 `B13B2DA5CFC74B5D1529200749C72A865FE46A4043F1B3CEFFC0E72DF16B19B7`，DLL 为 `2ECB974DD063C739F7991654286CF759A308CDE58B6E076A14DEB5AA2BE6A456`。需保留整包目录，不能单独拷贝 EXE。

## Flutter 现场批准与真实画面

实现提交 `f85dfe911`，手机为已有 API22 x86_64 模拟器。用户报告“连接上了”后实测核验，Flutter 显示“连接已获本机批准”，活动先记录“已提交本机批准，等待确认”，再记录“被控端已确认本次连接批准”。这次批准由本机用户完成，没有自动代点。

手机原生 XComponent 显示真实 Windows 主屏，工具面板读到 **1280×720、接收 2037 帧 / 呈现 2032 帧、9816 KiB**；截图可见当前 Windows 工作台和模拟器的递归画面。两个计数独立限频更新，差值不能直接视为丢帧率；这不是性能基准。Windows CM 不含视频状态字段，仍只陈述连接获批，由手机实际画面和统计证明视频通过。

点击 Windows Flutter “结束本次连接”后，手机显示连接已结束，XComponent 数量为 **0**，Flutter 随服务回执显示“本次连接已结束”，工作台保持可见。再由手机点击重新连接，双方回到新的待批准状态，截图无旧桌面画面，没有复用上次授权。等待阶段会预建一个待显示 XComponent；不将组件存在当作收到视频，也不将未读取的统计宣称为 0 帧。

另用新请求验证关闭工作台：请求于本机日志 UTC 06:51:24 发起，关闭窗口后 UTC 06:52:05 手机已显示“无法连接被控端”，XComponent 为 **0**，Flutter 窗口及对应进程退出。观察间隔小于 60 秒批准超时，与先前自然超时区分。该项覆盖待批准连接的关窗清理；**活动视频期间直接关窗、拒绝按钮实际点击、长时间资源曲线仍未验收**。

新增本机证据：`flutter-host-approved.png`、`flutter-phone-approved.jpeg`、`flutter-phone-video-stats.jpeg`、`flutter-host-active-ended.png`、`flutter-phone-host-ended.jpeg`、`flutter-host-reapprove.png`、`flutter-phone-reapprove.jpeg`、`flutter-phone-window-cleanup.jpeg`。截图不进入 Git。

收尾恢复可见的 Flutter 工作台，手机返回设备页，无活动连接，服务仍只监听 `127.0.0.1:21120`。PC 模拟器保持关闭。本轮仅补验证文档，运行的 EXE/DLL 与上述构建校验值不变。

## 独立复核

补充收口验收（2026-10-03，实现仍为 `f85dfe911`）：用户再次现场批准后，手机已有真实 Windows 画面和原生 XComponent；直接关闭 Flutter 工作台，手机立即显示“连接已结束”，XComponent 为 0，原 CM 进程退出。活动视频关窗清理已实测通过。

拒绝按钮另用新请求实测：UTC 15:05:04 发起，15:05:46 已结束，小于 60 秒批准超时。Flutter 活动明确记录“已提交拒绝请求”再记录连接结束；手机显示通用“无法连接被控端”并清屏，XComponent 为 0。此前控件索引失效和等待自然超时的尝试不算拒绝成功，先前口头表述已纠正。当前手机不区分本机拒绝与其他连接失败，未伪造专用拒绝原因。证据为 `flutter-active-before-window-close.jpeg`、`flutter-active-window-closed.jpeg`、`flutter-phone-reject-verified.jpeg`。

至此本轮界面验收中拒绝、活动视频关窗两项已补齐；长时间资源曲线、真机、公网仍未验收。随后独立实施键鼠切片，见 [输入计划](../plans/2026-10-03-system-input.md)。

修正了旧 CM `authorize` 乐观更新：专用 Flutter 窗口只发送批准，收到严格服务 Login 回执后才更新已批准；普通 CM 路径不变。CM 数据不含视频协商和首帧状态，因此窗口只声明连接获批，不将其等同画面正在传输，视频权限注明须本机启用并完成本次协商。

Windows runner 原有参数修剪删除末个有效字符，直接阻塞 `--cm` 识别；本轮只增加 `+ 1` 修正边界，实际专用窗口启动与待批准接线已验证。

## 回归影响范围

- `flutter/lib/main.dart` 仅专用 CM 增加能力核验、页面与窗口尺寸设置；普通页面仍沿原路径。
- `flutter/lib/models/server_model.dart` 仅专用模式跳过空请求时自动隐藏/退出、密码轮询和已批准后自动最小化；保留原模型和点击保护。
- `flutter/windows/runner/main.cpp` 参数尾部修剪修复影响所有参数读取，是专用 `--cm` 入口可运行的必要修正。
- Rust `lib.rs` 只增加条件模块；`connection.rs` 与 `ui_cm_interface.rs` 在 Windows + Flutter + ord 构建/显式专用 CM 进程选择独立 IPC。普通与 feature-off 仍使用 `_cm`。
- `ui_cm_interface::authorize` 专用进程等待真实回执；`secure_host.rs` 仅 Flutter 在现有批准、权限快照、可选视频初始化成功后发送既有 CM Login 回执。准入门禁和输入权限没有放宽。
- `flutter_ffi::cm_get_config` 增加只读模式能力查询，桥接签名不变；pubspec 测试锁调整如上，子模块和 Cargo.lock 不变。

用户签名、`.clangd`、`.clang-tidy` 与原型共 16 个保护文件 SHA256 已核对无变化；生成桥接、SDK、缓存和截图不提交。

## 固定边界

复用既有严格 IP、v1 加密、逐连接本机批准和 VP8 只读屏幕。视频默认关闭，不自动批准、不扩大监听、不开放系统键鼠/文件/无人值守，不修改系统安全设置。PC 模拟器保持关闭，后续联调使用手机模拟器；真机验证继续后移。
