# 浮球面板内部按钮命中修复

日期：2026-10-09。基线：`1fc53a636`。用户报告浮球菜单各按钮均无反应。

## 复现与原因

在 HarmonyOS 7 / API 26 真机通过 hdc 打开菜单，点击「状态详情」和「触屏模式」。按钮均为 enabled，但状态详情没有打开、模式没有切换，菜单仍保持原样。该结果排除了单个动作授权不足的解释。

面板最外层 Column 使用 `HitTestMode.Block`。官方定义该模式只让自身响应，阻塞子节点、兄弟与祖先节点的命中，导致标题关闭按钮、Scroll 和全部内部 Button 都被挡住。此前只验证浮球自身和菜单展开，未覆盖内部按钮，漏掉此问题。

修复仅将 `toolsPanel` 外层改为 `HitTestMode.Default`。该模式允许自身及子节点响应，同时阻挡下层兄弟节点。保留 Touch/Mouse 的 `stopPropagation`、全屏关闭遮罩的 Block，以及菜单开启时远端输入的 `!toolsOpen` 门禁；浮球自身与独立还原按钮仍使用 Block。

官方资料通过 HarmonyOS knowledge MCP 检索并读取：[交互基础机制说明](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-interaction-basic-principles)、[触摸测试控制](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-attributes-hit-test-behavior)、[HitTestMode](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-appendix-enums#hittestmode9)。Default 与该属性自 API 9 支持，符合项目最低 API 22。事件先完成命中收集，再分发触摸和手势；终止冒泡不会抹掉已经收集的子 Button 点击。

## 验证与交付

- 修复前真机对照：`DetailsPageOpened=false`、`OriginalActionStillVisible=true`、`ModeChanged=false`、`MenuStillOpen=true`。
- 33 项现有 Node 回归通过，Hvigor assembleHap 构建成功，独立只读检查确认最小修复及防穿透语义。
- 使用原调试身份签名，verify-app、原生 codesign、verify-profile 全通过，当前设备绑定匹配。最终 HAP SHA-256：`95c57b370f093d1a0d5eaeb531ffa588ff080d4049e862383216007a528c25a2`。
- 真机覆盖安装和启动成功，用户已批准新的 Windows 会话。用户明确选择自己测试，因此停止后续自动 UI 点击；修复后的各按钮实际操作结果等待用户反馈，不能写成已逐项通过。

产物仅保存在忽略的 `apps/harmony-controller/artifacts/orb-buttons-20261009/`。本轮没有为单一 ArkUI 属性增加镜像源码的单元测试或测试框架；真机前后对照是该问题的有效验收方式。

## 回归面

生产代码只有 `RemoteSession.ets` 的 toolsPanel 外层一处命中模式变化。未修改按钮业务、ready/输入授权、共享画布、网络协议、Windows 被控端、签名或本机 SSH 配置。Windows 不需重新打包。
