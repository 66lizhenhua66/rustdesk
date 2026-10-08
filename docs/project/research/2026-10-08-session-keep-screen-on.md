# 鸿蒙远程会话屏幕常亮

2026-10-08。用户要求远程连接时保持屏幕常亮；只控制当前应用主窗口，不修改系统全局熄屏时间。

## 行为与实现

`RemoteSession` 在 `sessionVisible && surfaceActive && screenIsLive(screen)` 时请求常亮。连接中、身份核验、等待批准、已批准和查看属于现有活动会话状态；断开、失败、返回设备页或隐藏会话会释放常亮。既有 `Desk.onPageHide` 和应用退后台逻辑结束活动连接，继续通过状态更新释放常亮；组件销毁也显式释放。

新增 `SessionKeepScreenOn` 只在期望状态变化时工作。窗口获取完成后重新核对期望状态，已取消的请求不会再开启常亮；原生开关按 Promise 完成顺序串行处理，开启途中销毁也会在完成后关闭。失败记录日志，不阻塞连接或改变输入逻辑，不按视频帧反复重试。

`HarmonyKeepScreenOn` 从当前 UIContext 取得 UIAbilityContext，再使用 `windowStage.getMainWindow()` 取得主窗口。每个会话只取得一次窗口，避免依赖可能返回子窗口的最上层窗口接口。

## 官方依据

以下文档均已通过项目 HarmonyOS developer knowledge MCP 的 `searchDocuments` 检索，并用 `getDocumentsById` 读取全文。

- [Window.setWindowKeepScreenOn](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-window-window#setwindowkeepscreenon9-1)：Promise 接口从 API 9 支持，系统能力 `SystemCapability.WindowManager.WindowManager.Core`，没有额外权限声明。只影响当前窗口在前台时的设备常亮状态；退出使用场景应设置为 `false`。
- [UIAbilityContext.windowStage](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-inner-application-uiabilitycontext)：属性从 API 12 支持，仅限主线程使用。
- [WindowStage.getMainWindow](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-window-windowstage#getmainwindow9-1)：从 API 9 支持，适用于 Stage 模型，异步取得主窗口；建议在内容加载后调用。本实现由已挂载的会话组件触发。
- [控制亮度与常亮](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/brightness-control)：前台常亮窗口禁用超时自动熄屏；窗口退后台时系统自动释放其常亮锁。
- [如何防止视频播放时自动熄屏](https://developer.huawei.com/consumer/cn/doc/harmonyos-faqs/faqs-arkui-1499)：按场景开始、停止设置常亮，避免重复或错误开关。

以上 API 起始版本均早于项目 HarmonyOS 6.1 目标，无需新增权限或变更 manifest。

## 验证与范围

执行 `node --experimental-strip-types --test apps/harmony-controller/tests/session-keep-screen-on.test.ts apps/harmony-controller/tests/session-input.test.ts`：7 项通过。新增 3 项覆盖状态重复更新不重复调用、隐藏和销毁释放、窗口尚未取得即取消、原生开启尚未完成即销毁。`git diff --check` 通过。

现有文件回归面仅为 `RemoteSession.ets`：新增常亮服务生命周期和状态通知，原有输入处理、画布、网络与授权路径保持原样。新增两个服务文件和一份测试。已与 [触控抬手修复](2026-10-08-harmony-touch-release-fix.md) 合并完成 30 项 Node 回归、HAP 构建、既有身份签名验证和 HarmonyOS 7 / API 26 真机覆盖安装、启动。

真机仍需验证：连接界面保持前台并静置超过设备原熄屏时间；随后分别断开、隐藏会话和退后台，确认恢复系统原有熄屏行为。自动化没有代替该设备验收。
