# 鸿蒙会话输入适配

共享输入实现已迁入 Rust，不能把本目录当作跨平台引擎。入口与各端状态见 [SPEC-004](../../../../../../../docs/project/specs/004-cross-platform-input.md)。

| 要修改的内容 | 位置 |
| --- | --- |
| 触摸点按、长按、拖动、双指手势 | libs/controller-core/src/interaction.rs |
| 坐标、鼠标按钮、滚轮、物理键保持、Unicode 分块 | 同一 Rust 交互引擎 |
| 版本化事件/回调/生命周期契约 | libs/controller-core/include/input.h |
| 鸿蒙事件来源、KeyCode、事件传播 | HarmonyInputAdapter.ets |
| 鸿蒙模式选择、界面状态和原生桥调用 | SessionInputController.ts |
| 鸿蒙系统输入法绑定与确认文本 | ../service/RemoteInputMethod.ets |
| Flutter Windows/Android/iOS 事件与 FFI | flutter/lib/controller_input/ |

鸿蒙链路为 ArkUI → HarmonyInputAdapter → SessionInputController → Desk → NAPI → 共享 Rust 引擎 → 既有严格会话队列。NAPI 每个 Job 延迟创建一个引擎句柄，调用与 reset 在 JS 线程串行执行，Job 销毁时 free。生成命令的 sink 直接进入 controller_session_send_input_v1，不让 UI 生成或持有授权令牌。

InputEvents.ts 只声明标准事件载荷；坐标是实际画面组件内的逻辑单位，尺寸与坐标单位一致。物理键同时传 opaque physicalCode 和远端语义 code。触点和按下状态只由 Rust 持有，鸿蒙不复制手势算法。

本目录的 ScreenState、中文提示、默认模式和系统键盘属于鸿蒙 UI 适配；它们不进入 Rust/C ABI。真实授权在共享会话核心及被控执行点。输入事件错误时 NAPI 清本地引擎并关闭输入，关闭失败取消会话；UI 同步回到不可输入状态。release 通知共享引擎释放真实持有的按键；reset 只清本地状态，用于撤权或断开。

后续增加手势策略先在共享 Rust ABI 测试中断言生成命令，再通过 Flutter 真 DLL 测试验证跨语言接口；鸿蒙 Node 测试只检查状态门禁与标准事件转发。平台输入法候选只在确认后交给 text 事件，Unicode 分块由共享引擎处理。
