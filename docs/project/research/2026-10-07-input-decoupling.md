# 输入解耦记录

日期：2026-10-07。本轮基于同一工作区已实现的直接控制交互继续解耦，不以 Git HEAD 的旧面板实现作为行为基线。

## 结果

`RemoteSession.ets` 从 663 行减至 395 行。触屏和物理键鼠分别适配，页面只绑定事件、提供焦点/键盘出口、更新会话和画面尺寸、渲染状态。平台外的输入控制与手势逻辑使用纯 TypeScript，不依赖 ArkUI、NAPI 或页面实例。

统一输入控制器复用现有 `SessionInputMode` 与远端 `ScreenState`，统一所有来源的发送门禁、失败处理和释放。按下状态分别属于实际持有它的触屏/键鼠适配器；统一 reset 清除所有来源。没有增加插件注册、消息总线或另一套远端授权状态。平台事件与 KeyCode 转换集中在 `HarmonyInputAdapter.ets`，本轮没有引入新的 HarmonyOS API。

后续触控手势、鼠标滚轮和组合键可在各自模块修改，并从公开控制器入口验证实际发送序列，详见 [输入模块说明](../../../apps/harmony-controller/entry/src/main/ets/input/README.md)。

## 验证与边界

- 新公开接口测试在模块尚未实现时失败；实现后触摸、物理键鼠、统一门禁三组行为测试通过，断言实际命令与用户可见状态。
- 移除两项由新接口测试覆盖的 `SessionInputMode` 内部测试；现有模型加新行为测试共 24/24 通过。没有安装测试依赖，Node 24 内置解析钩子仅用于解析 ArkTS 的无扩展名导入。
- DevEco SDK 6.1.1.125/API24 的 Hvigor `assembleHap` 通过。保留现有可抛异常 API 提示；未改变 IME 异常处理。
- API22 手机模拟器覆盖安装、启动成功，已有设备资料保留。此结果不代表真实远端输入或真机手势体验已验收。
- 独立审查逐路径对照重构前快照，未发现本轮新增问题。视口更新只更新坐标尺寸，面积变化时沿用页面原有释放通知，避免额外增加释放时机。
- 证据在忽略目录 `apps/harmony-controller/artifacts/`：`input-decoupling-tests.log`、`input-decoupling-hap.log`、`input-decoupling-workspace.json`；重构前对照为 `input-decoupling-RemoteSession.before.ets`。

触摸 6 vp 拖动门槛、500 ms 长按判定、滚动 24/40 vp 步长、共享滚动余量、左右修饰键最后抬起才释放、鼠标合成 Touch 过滤及取消处理均按前轮实现保留。系统键盘仍通过已实现的独立模块进入同一命令出口。

## 本轮回归面

既有运行文件只调整 `RemoteSession.ets` 的输入接线：事件处理、模式操作、焦点、尺寸变化和页面生命周期改由新控制器执行，这是把输入移出页面所必需的改动。测试文件 `input-models.test.ts` 去除重复内部测试，文档同步新位置。

本轮没有追加 Rust 协议、Windows 执行器、Desk 连接调度、IME 服务或本地签名配置的修改。这些文件在工作区保留的前轮改动仍存在，不属于本次解耦增量。默认控制、仅查看、可信只读限制和现有网络通道继续沿用。
