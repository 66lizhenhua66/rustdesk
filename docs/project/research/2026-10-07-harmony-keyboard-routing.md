# 鸿蒙键盘、输入法与系统快捷键

用户报告：使用鸿蒙星闪键盘时，Alt+Tab 切换的是鸿蒙应用，没有操作 Windows。2026-10-07 通过项目 HarmonyOS 官方知识 MCP 检索并读取下列原文，核对项目 target API24 / compatible API22。未据此宣称星闪真机或系统拦截已验收。

## 已确认的 API 行为

| 项目 | 官方结论 | 对本项目的影响 |
| --- | --- | --- |
| `keyboardShortcut`，API10 | “以下组合键绑定为快捷键不生效”包含 Alt+Tab、Alt+Shift+Tab、Alt+F4、Ctrl+Shift+Esc | 不能通过普通组件快捷键注册抢占这些系统行为 |
| `onKeyEvent` boolean，API15 | 返回 true 表示事件已消费并阻止冒泡 | 只能处理已经投递到 ArkUI 的事件，不能取回系统先消费的组合键 |
| `onKeyPreIme`，API12 | 返回 true 会拦截后续页面快捷键、IME、onKeyEventDispatch 与 onKeyEvent | 原始键模式可在 IME 前转发后消费，避免本地 IME 改写；文本模式返回 false 交给本地 IME |
| 外接键盘 | 设备按键经驱动/多模转换，发送至当前获焦窗口 | 未找到星闪键盘需要专用应用按键 API 的依据；仍需实际型号的事件/IME 行为验证 |
| `OH_Input_AddKeyEventInterceptor`，API12 | 文档举例“云桌面应用需要拦截按键、鼠标、触摸和轴事件”；仅应用获焦时拦截 | 符合远程桌面用途，应作为系统键捕获路线评估 |
| `ohos.permission.INTERCEPT_INPUT_EVENT` | system_basic / system_grant；受限申请场景包含“云桌面或是远程登录客户端” | 需要申请、签名 ACL 与审批，普通 manifest 声明不等于已经获权；获权后仍须专测 Alt+Tab 的优先级 |

API21 的 `OH_Input_AddKeyEventHook` 不能当本项目的通用替代：它依赖的 `HOOK_KEY_EVENT` 受限许可场景列为证券和网银等登录、交易、身份认证，不能借用其他类别申请。窗口级 filter 只拦截已经经过窗口的事件，官方未保证能拿回系统已消费的快捷键。API26 的新输入监听接口不属于本项目当前版本可依赖的方案。

## 本轮接线与明确缺口

远程 Surface 增加 `onKeyPreIme`，与已有按键适配共用唯一门禁：原始键模式中已识别的按键转发后返回 true，后续 onKeyEvent 不再重复接收；打开本地输入法时，现有 keyboardOpen 门禁返回 false，让系统 IME 完成组合并通过 `insertText` 发送确认文字。原始按键与确认文本不得同时发送同一个字符。该接线已具备官方 API 依据，实际星闪设备需验证。

当前没有接入原生 interceptor，也没有申请或修改受限权限。Alt+Tab 的当前故障没有因此被宣称修复：普通应用的 pre-IME 分流也不能保证抢在系统全局快捷键之前。完整捕获需要先完成受限权限流程，再实现仅前台控制状态启用、模式/焦点变化移除、失败回退、取消和修饰键释放的适配，并在目标设备测试。

可用性替代方案是在会话工具栏提供明确的“发送 Alt+Tab”等远端快捷操作，通过正常授权通道合成组合键；这是后续交互选择，本轮尚未增加按钮或新的控制面板。

## 真机核查方法

在专用测试窗口记录设备型号、系统/API、连接方式、应用焦点、原始/本地文本模式，以及事件到达了 pre-IME、IME 提交或 onKeyEvent 哪一层。仅记录按键种类/动作和阶段，不记录用户实际文字、剪贴板或密码。分别检查星闪/蓝牙/USB 的普通键、左右修饰键、中文候选、Alt+Tab、Win/Meta、长按重复、断开键盘及失焦释放。没有事件到达应用时，不以 Windows SendInput 成功测试替代入口问题。

## 官方来源

- [组件快捷键与禁止组合](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-keyboardshortcut)
- [按键事件与 pre-IME](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-key)
- [键盘事件数据流](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-interaction-development-guide-keyboard)
- [原生事件拦截开发指南](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/interceptor-guidelines)
- [OH_Input_AddKeyEventInterceptor](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/capi-oh-input-manager-h#oh_input_addkeyeventinterceptor)
- [受限权限](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/restricted-permissions#ohospermissionintercept_input_event)
- [inputConsumer 的系统快捷键占用错误](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-inputconsumer)

设计图见 [控制输入流向](../INPUT-FLOW.md)。
