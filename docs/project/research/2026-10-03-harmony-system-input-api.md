# 鸿蒙系统输入控制端 API 依据

日期：2026-10-03。对应 [系统键鼠计划](../plans/2026-10-03-system-input.md)。本轮通过项目 `harmonyos_developer_knowledge` 实际调用 `searchDocuments` 和 `getDocumentsById`，查询仅包含 API 名与技术关键词。工程目标为 HarmonyOS 6.1.1/API24，兼容 API22；未修改签名、系统权限或原型。

| 官方资料 | 已核验约束与本轮使用 |
| --- | --- |
| [触摸事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-touch) | 基础 API7；TouchObject 的 id 标识触点，x/y 为组件内 vp；touches/changedTouches 要判空。触控板跟踪 Down/Move/Up，Cancel 清理输入。触控板放在滚动控件之外，避免父 Scroll 抢占滑动。鼠标左键也可转换为触摸事件，不重复绑定另一条左键发送通路。 |
| [按键事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-key)、[KeyCode](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-keycode) | 基础 onKeyEvent 为 API7，返回 boolean 的消费重载为 API15；仅获焦触控板接收物理键。按键表包含 Space，仅映射协议白名单；KeyEvent.keyText 是名称，unicode 不支持完整中文，不用它发送文本。 |
| [TextInput](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-basic-components-textinput)、[文本公共接口](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-text-common) | onChange 的 value 为正式上屏文本，previewText 为预上屏内容。enablePreviewText/PreviewText 为 API12；保留本地输入框，仅用户确认发送时提交正式文本，预编辑期间禁用发送与快捷键，最大 512 UTF-8 字节。 |
| [FocusController](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-uicontext-focuscontroller)、[焦点控制](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-attributes-focus) | requestFocus/clearFocus 为 API12，focusOnTouch 为 API9。触控板明确获焦；辅助按钮不抢触摸焦点，失焦释放按下状态。 |
| [管理软键盘](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-manage-keyboard) | 原生输入框获焦由系统绑定输入法，TextInputController.stopEditing 可退出编辑。软键盘造成的同宽高度变化不当作旋转；窗口宽度变化时暂停控制。 |
| [UIAbility 生命周期](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/uiability-lifecycle)、[UIAbility 与 UI 同步](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/uiability-data-sync-with-ui)、[@StorageLink](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-state-management-storagelink) | onBackground 释放资源且不执行耗时任务；AppStorage 与 API7 StorageLink 同步前后台状态，让 Desk 清理显示和输入状态，同时保留已有 native dispose。返回前台不沿用连接许可。 |
| [轴事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-axis) | onAxisEvent 为 API17，针对鼠标/物理触控板，不是手机屏幕双指事件。本轮使用明确的滚轮按钮，每次一格；未宣称实现双指滚动或物理滚轮监听。 |

视频保持唯一原生 XComponent，像素不进入 ArkTS/JSON。新增小型输入命令由 NAPI 交给 Rust；授权令牌始终留在 Rust。UI 只根据经过认证的 `input_state` 回执更新输入许可，视频统计和呈现事件不更改许可。查看模式、输入面板关闭、会话被工作台遮挡、窗口旋转、触摸取消、失焦、后台或断开都会停止相应捕获并释放本连接输入；Windows 执行点仍负责最终授权检查与清理。

触控板只发送按实际显示区域尺寸归一化的相对增量 `move_relative`，不维护预测的 Windows 光标位置。连续触摸增量按 20ms 合并，在抬手、按钮/按键操作前提交剩余增量；清理时丢弃剩余输入并释放。真实位置由 Windows 在执行点读取并限制在主屏，界面不绘制假光标。滚轮按钮每次一格，保持 core 的有界队列及失败即暂停语义。

本文是文档依据和实现约束，不是构建、模拟器、真实 Windows 输入或真机通过的声明。显式文本提交与原型的隐藏即时输入体验不同；本片不实现即时输入法跨端同步、剪贴板、组合键编辑面板或系统保留快捷键。

## 2026-10-04：控制端能力开关

用户已将流程调整为 Windows 只批准接入，接入后的键鼠开关由控制端决定，见 [能力开关计划](../plans/2026-10-04-controller-capabilities.md)。鸿蒙新增项目 NAPI `setInputEnabled(taskId, boolean)`，不新增系统 API 或权限；以上 ArkUI API22 兼容依据保持适用。

控制与键盘入口以“已接入、支持该能力、已有画面”决定是否可点；点选只发送开启请求，`controlMode` 表示本地意图，只有真实 `input_state` 确认启用后才显示可发送输入的面板。复用 `ScreenState.code` 的 `INPUT_ENABLING`/`INPUT_DISABLING` 表示排队后的等待，视频统计/呈现不覆盖最近的能力状态码，也不推断授权。请求 ID 与令牌由 Rust 管理，界面不增加计数器。

切回仅查看、关闭输入面板、隐藏会话、旋转或取消先停止本地发送，再调用关闭接口；后台与断开同时执行连接取消作为清理后备。关闭回执到达前显示关闭中，不把本地意图写成服务端已关闭。切换已开启的触控板/文字面板仅释放当前按下状态，不重复关闭/开启。声音、文件与跨端剪贴板仍未实现。
