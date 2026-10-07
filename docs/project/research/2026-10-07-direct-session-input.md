# 远程画面直接控制：实现与验证

日期：2026-10-07。基线 `2d2a1ff87`，本轮为工作区改动。

## 用户要求与实现

用户确认普通连接分控制与仅查看，默认控制；不再使用额外控制面板，键盘鼠标直接映射到被控端。

Desk 提供进入模式选择及设备卡的控制/仅查看入口。RemoteSession 在批准、输入能力及首帧就绪后自动请求启用一次；输入仍等待真实回执，明确仅查看或启用拒绝不自动重试。已有活动设备显示「返回会话」，返回设备页会关闭输入；回到会话后可再次选控制。

现有等比 XComponent 直接接收触摸、鼠标、滚轮和键盘事件。触摸点按对应位置，滑动拖动，长按右击，双指滚动；黑边不产生点击。鼠标来源的伴生 Touch 被过滤，取消/失焦释放按键与按钮。工具栏或系统键盘收起不关闭控制，旋转释放按住状态并使用新几何。

系统键盘通过 `@kit.IMEKit` 自绘编辑客户端绑定，不订阅预编辑能力，仅将 `insertText` 的确认文字送往 Windows；同时转发删除、Enter 和方向键。文本按完整 Unicode 码点分为最多 512 UTF-8 字节的消息。绑定/显示途中关闭时停止发送并解绑。

正式输入协议与 Windows 执行器新增 F1—F12、常见标点、锁定键、Insert、左右 Windows 键和数字小键盘映射。必须同步更新两端：旧 Windows 产物不认识新增键码。操作仍受本机能力门禁、逐连接批准、授权令牌、失联清理和交互桌面检查约束。可信登记与无人值守仍只读。

## 验证

| 检查 | 结果 |
| --- | --- |
| 鸿蒙现有三组 Node 模型测试 | 23/23；新增坐标、默认启用及只读边界测试先失败再通过 |
| `cargo test --manifest-path libs/controller-core/Cargo.toml --locked --offline` | 108/108；新增物理键解析/协议测试先失败再通过 |
| `rustc --edition=2021 --test src/server/secure_input.rs` | 8/8；假注入器确认新增键的 Windows 按下/抬起映射，未注入真实桌面 |
| OHOS Rust release x86_64 / aarch64 | 两种 ABI 构建通过 |
| DevEco SDK 6.1.1.125/API24 Hvigor assembleHap | 通过；本机 product 未关联签名配置，产物为 unsigned HAP |
| Windows Debug Flutter 整包 | 通过；本轮只改 Rust 输入路径，未改 Flutter 页面 |
| API22 手机模拟器 | 覆盖安装及启动成功，布局与截图确认进入模式默认「控制」 |
| 独立差异审查 | 已修正鼠标 CANCEL 释放与活动会话入口文案，复核通过 |

本轮自动化合计 139 项，未重跑未受影响的 Flutter widget 与旧 DEMO 套件，因此不能把本数值与此前 157 项直接比较。Rust 原有告警仍存在；ArkTS 对 IME 可抛异常 API 发出提示，异常由串行操作的外层捕获处理。

构建首次受 Flutter/Hvigor 用户缓存权限限制；使用同一工具链及缓存重试。Windows 首次整包链接被旧测试进程占用，确认仅有回环监听且没有活动连接后停止这两个进程，重建成功并恢复 `127.0.0.1:21120` 的 `-Video -Input` 测试入口及 Flutter 工作台；未自动批准会话。

证据位于忽略目录 `apps/harmony-controller/artifacts/`：`direct-input-core-tests.log`、`direct-input-windows-tests.log`、`direct-input-hap-build.log`、`direct-input-hap-assemble.log`、`direct-input-windows-build.log`、`direct-input-workspace.json` 与 `direct-input-workspace.jpeg`。

## 回归面与待验

- `RemoteSession.ets`：普通会话默认模式、画面事件、输入法与输入释放，是用户要求的交互变更所在。
- `Desk.ets`：进入模式选择与参数传递、已有会话入口名称。
- `InputModels.ts`：新增坐标与模式模型；现有请求门禁显式排除可信只读会话。
- `RemoteInputMethod.ets`：新增的会话专用系统输入法适配，不影响设备资料编辑框。
- `libs/controller-core/src/input.rs` / `src/server/secure_input.rs`：仅扩展正式输入路径接受的键码及 Windows 映射；现有授权、释放和普通上游输入路径不变。

真机直接输入、中文候选与外接设备兼容性、滚轮灵敏度、旋转/后台/失联释放仍待 D06—D12 现场验证。模拟器本轮仅验安装启动及进入模式展示，没有借此宣称远端系统输入或真机验收通过；签名配置、已有本地改动及凭据未修改。

## 官方 API 依据

通过项目 HarmonyOS 知识 MCP 检索并读取官方文档，并与本地 SDK 声明核对：不设置 libraryname 的 Surface XComponent 自 API12 可使用 ArkUI 事件；onAxisEvent 自 API17，MouseAction.CANCEL 自 API18；输入法客户端绑定/监听自 API10，NEWLINE 自 API12，低于项目兼容 API22。

- [XComponent](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-basic-components-xcomponent)
- [鼠标与滚轮](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-interaction-development-guide-mouse)
- [自绘编辑控件使用输入法](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/use-inputmethod-in-custom-edit-box)
- [输入法客户端 API](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-inputmethod)

不把编译或文档核对当作实际输入法/外接设备行为验收。鼠标滚轮按官方轴方向转为远端整格滚动；触控板连续轴按显示距离累积，灵敏度需设备确认。
