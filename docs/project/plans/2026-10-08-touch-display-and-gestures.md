# 触屏展示与手势控制实施计划

> **面向 AI 代理的工作者：** 按任务顺序执行，每个任务完成后运行对应验证；不要把 Android/iOS 真机验证写成构建通过。

**目标：** 实现跨平台安全画布、1.0x—10.0x 本地缩放、旋转后保持缩放与中心，以及无冲突的触屏手势。

**架构：** 共享 Rust 输入引擎持有显示变换和触屏状态，平台适配提供窗口/安全区/触点数据；会话 UI 只展示状态和还原按钮。远程画面分辨率和 Windows 执行协议不变。

**技术栈：** Rust `controller-core`、版本化 C ABI、HarmonyOS ArkUI/NAPI、Flutter/Dart FFI、Node/Rust/Flutter 测试。

---

### 任务 1：共享显示变换模型

**文件：**
- 修改：`libs/controller-core/src/canvas.rs`
- 修改：`libs/controller-core/include/canvas.h`
- 测试：`libs/controller-core/tests/canvas.rs`

- [x] 增加 `SafeRect`、远程尺寸、`zoomFactor`、`panOffset` 和当前远程中心的纯数据模型。
- [x] 增加设置窗口/安全区/远程尺寸的版本化入口，验证有限数值、非负尺寸和 `1.0..=10.0` 范围。
- [x] 实现远程坐标到本地坐标、本地坐标到远程坐标、黑边命中检测和边界钳制。
- [x] 实现旋转后的中心重算：先得到旧远程中心，再用新安全区和缩放比例计算新偏移。
- [x] 添加测试：竖屏适应、横屏适应、黑边拒绝、10 倍缩放边界、旋转后中心保持和越界钳制。

### 任务 2：共享触屏状态机

**文件：**
- 修改：`libs/controller-core/src/canvas.rs`
- 修改：`libs/controller-core/include/canvas.h`
- 测试：`libs/controller-core/tests/canvas.rs`

- [x] 将单指状态拆成等待、画布平移、拖动准备和远程拖动；长按阈值固定为 1000 ms。
- [x] 将双指状态拆成右键点击、滚轮和捏合，距离变化优先于滚轮和右键。
- [x] 增加长按计时检查入口，返回“拖动准备”效果，不提前发送左键按下。
- [x] 添加测试：轻点、1 秒长按未移动、长按后移动拖动、两指右键、两指滚轮、捏合、取消和释放。

### 任务 3：鸿蒙展示接线

**文件：**
- 修改：`apps/harmony-controller/entry/src/main/ets/components/RemoteSession.ets`
- 修改：`apps/harmony-controller/entry/src/main/ets/input/HarmonyInputAdapter.ets`
- 修改：`apps/harmony-controller/entry/src/main/ets/input/SessionInputController.ts`
- 修改：`apps/harmony-controller/entry/src/main/cpp/controller_napi.cpp`
- 修改：`apps/harmony-controller/entry/src/main/cpp/types/libcontroller/Index.d.ts`
- 测试：`apps/harmony-controller/tests/session-input.test.ts`

- [x] 将 ArkUI 窗口尺寸、远程尺寸和窗口避让区域与实际视口交叠传入共享变换，叠加工具栏和 24vp 黑边。
- [x] 在 `SafeRect` 内渲染远程画面，保留黑边并忽略黑边输入。
- [x] 添加缩放状态展示和安全区内浮动“还原画面”按钮。
- [x] 旋转时取消手势、释放输入、更新变换并保留缩放比例与远程中心；已接真实系统 insets，设备呈现交由用户验收。
- [x] 将“拖动准备”状态映射成提示文案，不改变远程授权。
- [x] 更新 Node 测试验证画布配置、还原、状态和输入门禁；旋转与真实 insets 留给真机验收。

### 任务 4：Flutter 跨平台接线

**文件：**
- 修改：`flutter/lib/controller_input/controller_canvas_engine.dart`
- 修改：`flutter/lib/controller_input/flutter_canvas_input_adapter.dart 与 controller_canvas_view.dart`
- 修改：`flutter/test/controller_canvas_ffi_test.dart`

- [x] 将 Flutter 的窗口尺寸、安全区和远程尺寸转换为共享变换事件。
- [x] 保持 Windows、Android、iOS 使用同一缩放/旋转/手势引擎。
- [x] 添加 Flutter 真实 DLL 测试：10 倍缩放、旋转中心保持、还原按钮状态和长按拖动。
- [x] 明确 Android/iOS 原生库打包与真机测试仍是后续门槛。

### 任务 5：验收与文档

**文件：**
- 修改：`docs/project/PROGRESS.md`
- 修改：`docs/project/INPUT-FLOW.md`
- 修改：`docs/project/specs/005-touch-display-and-gestures.md`
- 修改：`docs/project/plans/2026-10-04-harmony-device-validation.md`

- [x] 运行 Rust 核心、鸿蒙模型和 Flutter DLL 测试。
- [x] 构建 OHOS 双 ABI/HAP；Flutter 组件测试覆盖安全区和缩放布局。实际鸿蒙横竖屏呈现保留给用户验收。
- [ ] 用户后续在 HarmonyOS 6.1 真机验证安全区、自动旋转、1x/10x、点击、拖动、滚轮和还原。
- [x] 把 D06—D10 的实现范围和限制写回真机清单；用户尚未实测的项目保留“待验证”。
