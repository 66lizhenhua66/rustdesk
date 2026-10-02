# 鸿蒙正式界面实现与验证

日期：2026-10-03。分支 `feat/harmony-controller`，起始提交 `181fe5c21`。对应 [实施计划](../plans/2026-10-03-native-controller-ui.md)。本轮完成原生界面和已有能力接线；本轮真实桌面人工批准后的复验仍待用户，不将上一轮结果或合成画面作为本轮现场验收。

## 原型与实现

已检查当前原型目录全部 HTML/CSS/JS、README/DESIGN，以及浏览器中的 A/B/C 设备页、手机/平板会话、工具、文件和 Windows 被控端。原型仍未最终选定 A/B/C，本轮按 README 推荐采用 A 设备工作台，不把设计实验切换器带入正式应用。原型所有文件保留原样。

- 正式入口改为 `pages/Desk.ets`。主绿、浅绿欢迎卡、显示器插画、白色圆角卡片、深色导航沿用原型；<600 vp 底栏单列，600–1099 vp 导航轨双列，>=1100 vp 完整侧栏与三列设备区。
- 沿用 `controller_profiles/profiles` 和 Rust 资料校验，提供新增、编辑、移除、公开配置预览导入、搜索与常用设备。IP/端口分字段，IPv6 正确加括号。编辑保留既有服务器资料，导入同本地 ID 不覆盖已存信任。
- 收藏和真实获批连接记录使用独立 `desk_favorites`、`desk_recent` 键。无样例设备、假在线状态、假延迟或假帧率；历史不保存口令、输入或画面。
- `RemoteSession.ets` 持有一个稳定 XComponent 显示节点。`Desk.ets` 持有连接与 Surface controller；每次连接创建新 generation 和组件 key，旧回调受 generation/taskId/页面可见性约束。返回工作台保留会话，第二目标需先断开。
- 连接严格使用 `expectedPeer=secure_video`、`minimumKxVersion=1` 和预存可信 ID/公钥。签名验证、加密通道、现场批准和首帧呈现分别显示；断开、取消、超时、失败清除 Surface 与本次批准。页面隐藏/后台仍取消并释放资源。
- 工具栏有手机双行/宽屏单行、收起胶囊、真实状态工具覆层与断开。短横屏的等待/结束卡将主操作固定底部；正文可滚动。视频按实际源比例适应显示区域。
- 系统控制、键盘、文件、无人值守、ID/中继、音频、剪贴板和画质切换明确未实现/禁用。没有接入正式键鼠，也没有将 DEMO 权限当作系统输入授权。
- 原 `Index.ets` 完整保留，从偏好设置“连接诊断”进入。网络预检、单次认证、正式非媒体入口及 Windows DEMO 路径继续可用；活动正式会话需先断开才能进入诊断。

Rust、NAPI/C++、视频 renderer 与 Windows 源码本轮均未改动。像素仍由原生 libvpx/NativeWindow 处理，不进入 ArkTS/JSON。没有新增权限，视频默认关闭和逐连接 CM 批准门禁保留。

## 自动化与构建

本轮 **82 项测试通过、0 失败**：

| 命令 | 结果 |
| --- | --- |
| `node --experimental-strip-types --test apps/harmony-controller/tests/desk-models.test.ts` | 5 组：批准前不显示画面、身份/登录门禁、实际计数、终止清零、IPv4/IPv6 字段转换 |
| `cargo test --manifest-path libs/controller-core/Cargo.toml --locked` | 63 项通过 |
| `cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked` | 9 项通过 |
| `scripts/build-official-secure-host.ps1 -TestVideo` | 4 项通过 |
| `scripts/build-official-secure-host.ps1 -TestSecureGate` | 1 项通过 |
| `scripts/build-official-secure-host.ps1 -DebugBuild` | 最终通过，保留原有编译警告 |
| `apps/harmony-controller/scripts/build.ps1` 及同工具链 Hvigor `assembleHap` | Rust/native 双 ABI、ArkTS 与 HAP 最终通过 |

原生构建脚本完成 x86_64 / ARM64 编译后，首次 Hvigor 因沙箱不能写用户缓存失败；使用现有用户工具链权限重试成功，没有改变签名、系统安全设置或工具链版本。初次 ArkTS 编译修正了保留成员名和 SelectOption 显式类型。后续模拟器检查修正了 Builder 值参数不刷新的按钮/统计、Stack 浮动控件定位、欢迎卡对齐与短横屏操作位置。

Windows Debug 首次更新 EXE 时被运行中的测试 server 占用。核实其为本仓库 `target/debug/rustdesk.exe --server`、仅回环监听且没有活动连接后停止该实例；最终增量构建通过，再用现有 `-Video` 脚本恢复 `127.0.0.1:21120` 与 CM。没有扩大监听、修改防火墙或自动批准。

产物：`apps/harmony-controller/entry/build/default/outputs/default/entry-default-unsigned.hap`。已检查归档包含 `libs/arm64-v8a/libcontroller.so` 和 `libs/x86_64/libcontroller.so`。未签名模拟器开发包，不等于真机签名或发布验收。

日志在本机忽略目录 `apps/harmony-controller/artifacts/ui-*.log`；最终 Windows 记录为 `ui-windows-debug-final.log`，HAP 为 `ui-hvigor-build.log`。

## 模拟器交互与截图

| 环境 | 实测范围 |
| --- | --- |
| 现有 OpenHarmony 6.0.2.130 / API22 x86_64 手机模拟器 | 1272×2756，约 376 vp 竖屏；旋转后约 816 vp 横屏。设备管理、状态页、原生 Surface 和工具栏 |
| 现有 MateBook Pro OpenHarmony 6.1.1.125 / API24 2in1 模拟器 | 3120×2080，约 1642 vp 最大化；原生拖动窗口边缘验证约 835 vp、498 vp。是 2in1 窗口尺寸测试，不是平板或真机认证 |

手机实际操作：非法 IP 拒绝并保留表单；添加临时设备、搜索、设为常用、改名及 IPv6 地址、冷启动读取、移除均通过。原有两台设备资料保留。公开配置导入先核验预览再保存，不自动建立会话。偏好设置正确说明未实现项；最近连接空态和旧诊断路由/返回通过。

通过设备传感器调试命令切换方向，并用真实布局和截图确认横竖屏变化；没有把缩放截图算作视口测试，也没有改变系统 DPI、旋转调试权限或安全参数。`orientation=auto_rotation` 使应用响应方向变化；它会忽略系统旋转锁，适用于本轮明确支持旋转的界面。

已与浏览器原型截图目视对照欢迎卡、导航、卡片留白、浮动栏/胶囊和工具覆盖布局。宽中窄窗口没有观察到横向溢出；长内容可滚动，短横屏取消/返回/重试可见。未将 599/600、1099/1100 的每一个精确边界都作为已实测宣称。

本机截图包括 `final-api24-wide.jpeg`、`final-api24-medium-final.jpeg`、`final-ui-home.jpeg`、`ui-add-device.jpeg`、`ui-form-error.jpeg`、`ui-settings.jpeg` 与 `final-ui-disconnected-landscape.jpeg`。迭代截图可能对应不同构建，最终截图/归档哈希在本机 `ui-api24-final-hap.json` 和验证日志中记录。

## 合成视频与正式入口的区别

使用原有 `libs/controller-core/examples/screen_fixture.rs`，仅回环传输 64×48 灰色 VP8 合成帧，经真实签名/v1 加密、Rust 和 native 解码到 Surface；**不抓取 Windows 桌面，也不实现 CM 现场批准**。临时资料明确命名 `Synthetic UI QA`，测试后移除其资料/记录及临时转发，保留原 21120 转发。

最终合成视频实测：

1. 首帧呈现后显示只读桌面界面，工具栏贴顶部，底部只读说明可见。
2. 工具面板显示实际 64×48、接收/呈现计数（当次观察 9 / 8，两个统计独立投递），后续继续更新；文件/控制/键盘禁用。
3. 收起为状态胶囊、再展开；横竖屏切换保持原比例和同一连接。
4. 返回工作台仍有同一 Surface 和活动目标横幅；返回会话继续显示。断开后 XComponent 数量为 0，终态按钮可见。
5. 对端结束产生 `failed/DISCONNECTED` 时按“连接已结束”显示，清零批准与统计；不再误写成尚未建立连接。

证据为 `final-ui-synthetic-portrait.jpeg`、`final-ui-synthetic-tools.jpeg`、`final-ui-capsule.jpeg`、`final-ui-synthetic-landscape.jpeg`；灰色是固定测试源，不能据此宣称真实 Windows 主屏验收。

正式入口已在新界面实际完成预存身份核验、加密和等待批准。等待中没有画面；旋转不重新连接；未批准超时后 Surface 清除、显示明确重试入口。已准备代码/构建/安装/CM 和请求并向用户请求本机接受，但截至本记录没有收到本轮接受完成反馈，因此批准后的真实 Windows 桌面、真实画面的跨页/断开/重连最终复验仍待用户。上一轮 1152×720、500+ 帧证据只见 [上轮报告](2026-10-02-readonly-screen-validation.md)，不算本轮通过。

最后对正式入口补验：等待批准时按 Home，再回到应用明确显示“应用已离开前台，连接已结束”；重试发起新的批准请求，横屏下返回/取消按钮始终可见，取消后 XComponent 为 0。证据 `final-ui-waiting-landscape.jpeg` 及 `ui-emulator-results.txt`。最终 HAP SHA256：`BFC57973E8AAAC5575CD3AA6FB60FD9876D243E24E0499C0C39E4E40562AFAB2`，手机与 2in1 均已安装此包。

## 官方 API 核验

本轮实际使用项目 Huawei MCP 的 `searchDocuments` 与 `getDocumentsById`，只传技术关键词并读取官方原文：

| 文档 | 本轮依据 |
| --- | --- |
| [组件区域变化事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-component-area-change-event) | 基础 onAreaChange API8；不使用 API26 interval/options 重载 |
| [XComponent](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-basic-components-xcomponent) | Controller Surface 回调 API12；Surface 创建后才连接；保持无 libraryname 的 SURFACE 路径 |
| [Preferences](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-data-preferences) / [持久化](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/data-persistence-by-preferences) | API9；put 后 flush 落盘；仅存非敏感资料，不以 Preferences 保存口令 |
| [自定义组件生命周期](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-custom-component-lifecycle) | onPageHide 包含进入后台；取消原生会话与旧 UI 回调 |
| [module 配置](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/module-configuration-file) / [窗口旋转](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/window-rotation) | Stage ability orientation 支持 auto_rotation，目标 API24、最低 API22 适用 |

## 最小回归面、保护与未完成项

独立源码复核检查了安全状态、异步任务、Surface 所有权、设备资料保存和 ArkUI 更新；最小化检查未发现必须改动 Rust/Windows 共享路径的理由。

- 既有 `EntryAbility.ets` 只更换默认页面；后台/销毁的 dispose 原样保留。
- 既有 `main_pages.json` 增加正式路由并保留诊断路由。
- 既有 `module.json5` 增加自动旋转，也适用于同 Ability 的旧诊断页面；它是横竖屏要求所需的唯一生命周期配置变化。
- 新 Desk/RemoteSession/DeskModels 负责本轮界面、非敏感存储与现有接口接线；未改变协议、媒体缓冲、Windows 安全入口或默认开关。
- 用户 `build-profile.json5`、`.clangd`、`.clang-tidy` 和原型文件按起始 SHA256 校验保护，不进入本轮提交。新增 SVG 被原有 ignore 规则忽略，须逐个纳入提交，不能修改 ignore 规则或漏交资源。
- 真机按用户要求后移。未验收真实输入、物理键鼠/IME、平板传感器/分屏、系统声音/文件、无人值守、ID/中继、公网或硬解；这些功能未被模拟成功状态代替。
- Windows 原型后续任务已列入计划，优先下一切片为独立本机授权、撤权及断开清理的系统键鼠，不在本轮 UI 中提前开放。
