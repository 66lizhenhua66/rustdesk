# 鸿蒙会话输入适配

共享输入实现已迁入 Rust，不能把本目录当作跨平台引擎。入口与各端状态见 [SPEC-004](../../../../../../../docs/project/specs/004-cross-platform-input.md)。

| 要修改的内容 | 位置 |
| --- | --- |
| 触摸点按、长按、拖动、双指手势、缩放与旋转 | libs/controller-core/src/canvas.rs |
| 语义按键、物理键保持、Unicode 分块 | libs/controller-core/src/interaction.rs，由 canvas 复用 |
| 版本化事件/回调/生命周期契约 | libs/controller-core/include/canvas.h 和 input.h |
| 鸿蒙事件来源、KeyCode、事件传播 | HarmonyInputAdapter.ets |
| 鸿蒙模式选择、界面状态和原生桥调用 | SessionInputController.ts |
| 鸿蒙系统输入法绑定与确认文本 | ../service/RemoteInputMethod.ets |
| Flutter Windows/Android/iOS 事件与 FFI | flutter/lib/controller_input/ |

鸿蒙链路为 ArkUI → HarmonyInputAdapter → SessionInputController → Desk → NAPI → 共享 Rust 引擎 → 既有严格会话队列。NAPI 每个 Job 延迟创建一个引擎句柄，调用与 reset 在 JS 线程串行执行，Job 销毁时 free。生成命令的 sink 直接进入 controller_session_send_input_v1，不让 UI 生成或持有授权令牌。

InputEvents.ts 只声明标准事件载荷；坐标来自未缩放 viewport，和 canvas snapshot 的 imageX/Y 使用同一原点。尺寸仅通过 configure 传递，touch/mouse/wheel 不再携带 width/height。CanvasModels 解析真实快照、计算系统避让区和实际视口的交叠，CanvasAvoidArea 监听窗口区域变化。物理键同时传 opaque physicalCode 和远端语义 code；触点和按下状态只由 Rust 持有。

本目录的 ScreenState、中文提示、默认模式和系统键盘属于鸿蒙 UI 适配。真实授权在共享会话核心及被控执行点；本地查看手势不依赖远程输入许可。输入事件错误时 NAPI suspend 本地引擎并关闭输入，关闭失败取消会话。release 释放真实持有的按键；suspend 禁用输入但保留缩放/画布位置。定时器按 nextTickMs 安排，失焦、隐藏、撤权时取消。

tests/fixtures/canvas-events.json 同时被 Node 实际发包测试与 Rust ABI 测试读取，固定跨语言事件契约，避免两边各自通过却无法互通。

后续增加手势策略先在共享 Rust ABI 测试中断言生成命令，再通过 Flutter 真 DLL 测试验证跨语言接口；鸿蒙 Node 测试只检查状态门禁与标准事件转发。平台输入法候选只在确认后交给 text 事件，Unicode 分块由共享引擎处理。
