# 画布与触屏集成交付记录

2026-10-08，基于 `ad813adc8` 修复并补齐 SPEC-005。用户要求先全部修改，再由用户真机验收；本轮没有将自动化结果当作真机通过。

## 交付行为

- 触屏模式：轻点左键、单指滑动画布、静止 1 秒显示拖动准备，之后移动拖动远程对象；长按后直接抬手不点击。
- 双指轻点右键；移动超过 8vp 取消点击资格，滚轮按 24vp 累积；捏合锁定本地缩放 1～10 倍，剩余单指平移不跳动，三指取消。
- 指针模式：相对移动鼠标、轻点左键、双指右键/滚轮、长按拖动；捏合仍调整本地显示。
- 画布按比例 fit，顶部/底部/左右安全区域由真实避让矩形与 viewport 求交，再叠加工具栏、状态条及 24vp 黑边。已由 ArkUI 避让的区域不重复扣除。
- 旋转保留相对 fit 的缩放倍数和远程归一化中心；超界取最近合法位置。零安全尺寸保持裁剪为空。
- 放大后显示还原按钮，恢复 1 倍居中；不关闭输入能力，但取消手势并释放持有按钮。
- 仅查看仍能本地查看、缩放、平移和还原；关闭控制、发送失败、失焦释放输入且保留画布缩放。
- IME 的 FunctionKey 只有 enterKeyType：NEXT/PREVIOUS 在应用侧映射 Tab/Shift+Tab，Enter 类动作发送 Enter，NONE 和未知值不发送。它不是一个包含 Escape/Tab 硬件键码的事件。

## 实现与修复

修正了鸿蒙触摸事件带 width/height 而 Rust 严格拒绝、缩放组件局部坐标误作 viewport 坐标、有限滚轮值被拒绝、sink 首次失败仍继续发送，以及多指误点击/计时器残留等问题。

所有指针事件绑定未缩放的整个 viewport。XComponent 只显示，按 fit 基础尺寸绘制并用 scale 放大，父安全区域裁剪；渲染和逆坐标来自同一 Rust 状态。按钮/工具栏作为事件视口的兄弟组件。新 `controller_canvas_suspend_v1` 在已撤权/关闭输入路径清本地状态，不销毁画布，避免每次切查看回到 1 倍。

Flutter 新增 Engine、Adapter 和 CanvasView，调用同一 C ABI。View 使用 MediaQuery 安全区、实际 viewport、叠加区域、状态提示和单次长按定时器。父布局应随软键盘收缩；覆盖式键盘布局需要调用方先约束画布可见区域。组件已可复用，完整 Windows/Android/iOS 控制端页面和平台库打包不属于本轮接入范围。

## 验证

| 检查 | 结果与证据 |
| --- | --- |
| Rust 完整核心 | 129 项通过，其中 18 项 canvas ABI 测试；正常滚轮、发送失败中止、短双指移动误右键等先红后绿 |
| 鸿蒙 Node 模型/接线 | 22 项通过；真实发包对照共享 fixture、安全区重叠、只读查看和定时器取消 |
| Flutter Windows 真实 DLL + widget | 新旧 8 项通过；1～10 倍、旋转中心、readonly、sink失败、实际MediaQuery、还原按钮和长按timer |
| Flutter 定向分析 | 无问题；保留旧远控运行路径 |
| 输入法公开回调 | NONE/未知、NEXT、PREVIOUS、NEWLINE、Unicode提交和解绑检查通过 |
| OHOS x86_64 / aarch64 + HAP | 构建通过；本机 product 未关联签名配置，产物为 unsigned HAP |
| 独立审查 | 短双指移动误右键与零安全区域回退问题修复后复核通过 |

共享事件样本 `apps/harmony-controller/tests/fixtures/canvas-events.json` 由 Node 发包检查与 Rust ABI 同时使用。它保证两层不是各自测试通过却使用不同字段。核心 socket 测试有一次既有 Windows 10053 偶发失败，单独及最终完整重跑通过。

证据日志位于忽略的 `apps/harmony-controller/artifacts/`：`canvas-completion-hap.log`、`canvas-completion-flutter-tests.log`；核心与 Node 完整命令结果在本轮执行记录。未安装新依赖，保留既有签名材料、构建脚本和 clang 配置。

## 用户真机验收顺序

1. 更新本轮 HAP 与匹配的 Windows 被控产物，完成一次本机批准。分别用竖屏、横屏看完整画面和安全黑边，检查工具栏/状态栏不遮住画面边缘。
2. 在专用窗口点击中心、四角和边缘按钮；黑边点按应无远端点击，按钮/还原操作不透传。
3. 捏合到 2 倍和 10 倍，单指平移看边缘文字；横竖屏切换后保留倍数与关注区域，一键还原后居中且连接仍有效。
4. 轻点、双击、双指右键、双指上下滚动；双指小幅移动后抬手不误右键，捏合不产生滚轮或右键。
5. 单指长按 1 秒看到提示后拖动远程文件/窗口；提示前移动只平移，长按后直接抬手不点击。拖动时增加第二/第三指、旋转、失焦或断开不会粘住按钮。
6. 切指针模式复测相对移动、点击、滚轮与拖动；切仅查看后只能缩放平移，不能改动远程桌面；恢复控制需真实启用回执。
7. 打开系统键盘输入中文/emoji，测试下一项/上一项/回车，收键盘后缩放与位置合理保留；外接键鼠失焦后无残留修饰键。

## 范围与限制

现有文件回归面限于 `canvas.rs/canvas.h` 的本次画布路径、鸿蒙会话/Desk/NAPI 的事件与清理接线、输入法未知功能键处理，以及 Flutter 新画布所需的既有键码转换函数可见性。新增平台安全区适配、画布模型、Flutter组件和对应测试。没有修改 Windows 注入器、网络协议、认证策略或原始 v1 interaction 实现。

Surface 合成缩放/裁剪、各机型挖孔及键盘、旋转观感和输入延迟仍需上述设备验收。动态远端分辨率改变仍受既有原生 renderer 限制。Alt+Tab 的系统级捕获仍需要受限权限及对应实现，本轮不宣称已解决；详见前轮键盘核查。Android/iOS 原生打包与完整生产控制端 UI 未由本轮 Flutter 测试代替。

官方依据已通过 HarmonyOS 知识 MCP 检索和全文读取：

- [默认安全区与避让](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-attributes-expand-safe-area)
- [窗口与 AvoidArea API](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-window-window)
- [Area/Position 单位及窗口坐标](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-types#area8)
- [XComponent 与 Surface 尺寸](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-basic-components-xcomponent)
- [绘制变换](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-attributes-transformation)
- [输入法 FunctionKey](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-inputmethod#functionkey10)
