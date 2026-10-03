# Windows Flutter 被控端原型界面实现计划

日期：2026-10-03。基线 `87a0aa459`，分支 `feat/harmony-controller`。正式 Windows 界面使用现有 Flutter 桌面工程；Sciter 仅保留此前已验证的开发壳，不再扩展。本轮未提交的 Sciter 页面和接线已撤回，用户原型、签名和工具配置保持原样。

## 目标与边界

在 `flutter/` 中还原当前原型的白色侧栏、浅灰绿主区域、双栏卡片、本机访问/访问记录/安全设置，以及真实请求、批准、拒绝、活动和结束状态。复用现有 Flutter Rust Bridge、CM 事件与 Rust 安全入口，不在 Dart 重写认证，不引入 WebView。

正式窗口通过 `--cm --ord-secure-ui` 显式进入，并核验 Rust DLL 的 Windows + flutter + ord-secure-host 构建能力。专用 IPC 避免旧 CM 截获请求；普通 Flutter、Sciter 开发壳和 feature-off 保持原路径。

视频仍默认关闭，只在本机显式 `-Video` 后请求查看；每次连接均需本机批准。系统键鼠、文件、无人值守、可信绑定、协助码、音频、剪贴板和提权均未实现/禁用。不得扩大监听、修改系统安全设置或代点批准。

## 页面与文件

| 文件/区域 | 职责与真实接线 |
| --- | --- |
| `flutter/lib/desktop/widgets/secure_host_workspace.dart` | 纯 Flutter 原型视图，接受不可变连接快照、活动记录与按连接 ID 的回调；本机访问/记录/设置、双栏/窄窗、固定批准/拒绝/结束栏 |
| `flutter/lib/desktop/pages/secure_host_page.dart` | 连接现有 ServerModel/FRB、订阅真实 CM 事件、等待批准回执、关闭清理、窗口控制；不触发权限开关 |
| `flutter/lib/desktop/secure_host_mode.dart` | 专用窗口模式的窄范围标记，默认关闭；由启动参数和真实 Rust 能力决定 |
| `flutter/lib/main.dart` | 仅专用 CM 选择新页面、可缩放工作台尺寸和可见任务栏；普通窗口配置保留 |
| `flutter/lib/models/server_model.dart` | 仅新模式跳过空列表隐藏/关闭、批准后自动最小化等旧窄 CM 窗口行为；原客户端模型保留 |
| `src/secure_host_ui.rs` 与 lib/IPC 薄钩子 | 仅 Flutter 正式构建的模式与 IPC 路由；不加载或嵌入任何 Sciter 资源 |
| `src/flutter_ffi.rs` | 在既有 cm_get_config 只读调用中增加专用构建能力查询，不新增桥接签名 |
| `src/server/connection/secure_host.rs` | 在原批准/权限快照/媒体初始化成功后，用既有 CM Login 事件回传已批准；不改准入条件、加密和执行点 |
| `scripts/build-windows-host.ps1` / `start-windows-host.ps1` | 按仓库 Windows x64 CI 的 Flutter 3.24.5 与 FRB 1.80.1 构建 DLL+Flutter EXE；显式回环启动，视频默认关闭 |

当前 `Client.authorized` 在旧 sendLoginResponse 中会乐观置 true。专用窗口批准沿原点击保护调用 cmLoginRes，界面先显示正在确认，等服务端真实回执后才显示已批准。CM 没有首帧数据，不展示假的缩略图、帧率或送达状态。

来源名称与 peer ID 为对方自报；新页显示此事实，不标控制设备已认证。本机 ID 沿现有 cmGetConfig(id) 只读获取。访问记录仅当前运行期的真实事件，不宣称持久化安全审计。

视觉按原型：白色204侧栏/窄窗72导航轨；主绿#196B52、文字#172C29、背景#F6F7F4、边框#E1E7E2，18圆角白卡和双栏约1.28:1。长内容滚动，批准/拒绝/结束按钮保持可见；不使用原型的示例密码、协助码、文件或权限成功状态。

## 实施与验收

- [x] 核对技术规格、当前原型、既有 Flutter CM/ServerModel/FRB，撤回未提交 Sciter 扩展。
- [x] 准备隔离的 Flutter 3.24.5 x64 工具链及固定生成工具，保持全局 SDK 与直接依赖/Git 固定提交；校验 SDK 和包校验和，仅调整 SDK 所需的测试依赖锁，不改系统设置。
- [x] 实现纯 Flutter 视图与三组 widget 行为测试：批准阶段与目标 ID、多请求/禁用能力、宽/窄/短窗口布局。
- [x] 实现 Flutter 模式、页面与现有模型的薄接线；Rust 仅专用路由、能力读取和批准回执。
- [x] 构建真实 Flutter Windows EXE 和匹配 Rust DLL；跑格式、定向 analyze/widget、既有核心/批准/视频回归及必要 feature-off 检查。
- [x] 实际运行 Flutter 窗口并截图对照原型；验证三页、无请求常驻、请求/结束、窗口缩放与关闭清理。
- [x] 保留已关闭的 PC 模拟器；用手机模拟器发起真实请求，窗口准备好后由用户本机批准；验收只读画面、断开清屏和重连重新批准。
- [x] 最小回归面与独立复核；更新进度/验证记录，本地提交、不推送，并刷新已有 GitNexus。

Windows runner 既有参数修剪会错误删除末个有效字符，直接阻塞 --cm 和正式入口参数；本轮仅补 +1 修复并以实际启动验证。

工具链缺口和首次构建失败须如实记录，不能用 Sciter EXE、独立 DEMO 或静态截图替代 Flutter 可运行交付。真机继续后移，不宣称真机/公网/系统输入通过。

现场补验（2026-10-03）：用户亲自批准后，Flutter 回执与手机真实主屏 1280×720 已核验；Windows 主动结束清屏、重连重新批准、待批准连接关窗清理通过。活动视频时直接关窗、拒绝按钮实际点击仍列为未验收，详见对应验证记录。
