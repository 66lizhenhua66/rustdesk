# Windows Flutter 被控端原型界面验证

日期：2026-10-03。基线 `87a0aa459`。对应 [Flutter 实施计划](../plans/2026-10-03-windows-host-ui.md)。本轮正式界面限定现有 Flutter Windows 工程，Sciter 开发壳不作为交付。

## 当前进度

此前本轮未提交的 Sciter UI 扩展已撤回。Windows 正式页面已在现有 Flutter 工程实现；复用 ServerModel/FRB 与严格服务，仅新增专用 CM 路由、能力查询和批准回执。鸿蒙页面、用户原型/签名/工具配置保持原样。

Debug 整包构建和本轮十项定向测试已通过，Windows 已实际打开 Flutter 工作台并收到手机模拟器的连接请求。现场批准和真实画面结果在下文单独记录。原始日志与截图留在忽略的 artifacts，含真实桌面的材料不进入 Git。

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
| 手机联调 | 手机 API22 模拟器已发起真实请求，Flutter 收到 Harmony Controller；未获本机批准而超时，手机提示批准超时，Flutter 显示连接已结束，XComponent 为 0。批准后的真实画面仍未验收，不用历史 Sciter 结果替代 |

桥接首次缺 `stdbool.h`，旧 ffigen 虽报告 fatal 却返回 0，错误生成遮蔽 Dart bool 的 typedef。显式传入 `--llvm-compiler-opts` 的 Windows target 和独立 `-resource-dir "路径"` 参数后，重新生成无 SEVERE/fatal，生成文件 analyze 零问题。脚本保留诊断检查，拒绝这种错误桥接。带空格路径不可用 `-resource-dir="路径"`，旧 ffigen 分词器会拆错。

最终日志：`flutter-build/windows-build-final.log`、`windows-tests-final.log`、`integration-analyze-final.log`、`frb-generate-final.log`。截图：`flutter-host-idle.png`、`flutter-host-records.png`、`flutter-host-settings.png`、`flutter-host-pending.png`、`flutter-host-timeout-ended.png`。

Debug 整包目录：`flutter/build/windows/x64/runner/Debug/`。本次 EXE SHA256 为 `B13B2DA5CFC74B5D1529200749C72A865FE46A4043F1B3CEFFC0E72DF16B19B7`，DLL 为 `2ECB974DD063C739F7991654286CF759A308CDE58B6E076A14DEB5AA2BE6A456`。需保留整包目录，不能单独拷贝 EXE。

## 独立复核

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
