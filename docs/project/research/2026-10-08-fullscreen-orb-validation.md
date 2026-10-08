# 整屏画布与侧边浮球交付记录

日期：2026-10-08。基线：`87ffc3d50`。用户已审阅可交互原型并选择侧边浮球，授权实施。

## 实现

画布显示和输入安全区分开：新增可选 `fullViewport` 配置，鸿蒙启用后使用完整 viewport 计算 fit，1× 保持远端比例并居中；真实系统 insets 仅限制远端输入与悬浮控件。Surface 使用 imageX/Y 绝对视口坐标，按整个屏幕裁剪。工具面板开关不改变 fit、缩放或位置。缺省 false 的其他调用保留原行为。

放大后的平移边界额外允许 24 vp，将边缘内容移入可点击的安全区；direct 模式仍不向远端发送图像外的黑边点击。旋转保持 zoom 和视口中心对应的远端位置；还原回到完整屏幕中心。安全区面积为零时禁用远端输入，仍保留本地画面。

原顶部工具栏替换为右侧 44 vp 可拖动浮球，点击向内展开工具面板；控制、仅查看、键盘、触屏/指针、还原、状态详情、返回和断开通过浮层操作。菜单外点击只关闭面板，输入层在菜单开启期间也停止接收事件；拖动浮球使用 windowY 和原生 4 vp 点击阈值，避免松手误开面板。放大后左上浮动还原按钮显示当前倍数。

Desk 会话页面以 `LayoutPolicy.matchParent` 与 `ignoreLayoutSafeArea` 实际扩展整窗，工作台保持原布局；键盘使用 `KeyboardAvoidMode.NONE` 覆盖画布，退出或退后台恢复原避让模式。保留已实现的前台会话常亮和现场批准门禁。

## 为什么 1× 横屏仍有黑边

手机物理屏幕 2832×1316，宽高比约 2.15；远端编码画面 1280×720，为 16:9。1× 完整显示远端时，画面高度使用全部 1316 像素，宽度约 2339.56，两侧自然各余 246.22 像素。约 1.21× 可以填满宽度，但会裁切远端上下部分，需要平移。这不是工具栏占位，也不是 1:1 像素模式。

## 自动化与构建

| 验证 | 结果 |
| --- | --- |
| controller-core 全部测试 | 133 项通过；3 项新增测试先失败后通过，覆盖整屏 fit/insets、旋转/reset、边缘平移和黑边点击 |
| 鸿蒙全部 Node 测试 | 33 项通过；含浮球安全区布局、拖动 ID/阈值和旋转相对位置 |
| OHOS 核心编译 | x86_64 和 aarch64 release 均通过，使用锁定离线依赖 |
| Hvigor assembleHap | 构建成功，保留原有 IME/AssetStore 异常处理警告 |
| 签名 | 原调试身份签名，verify-app、原生 codesign、verify-profile 通过；设备绑定匹配 |
| 独立审查 | 核心/Desk 与浮球 UI 分别复核，未发现阻塞问题 |

最终包 SHA-256：`3916194d03ebfa9e33bab0dfe7f9c5233d604891e93309daaa63c6a20069d9ff`，输出在忽略目录 `apps/harmony-controller/artifacts/fullscreen-orb-20261008/entry-default-signed.hap`。Windows 被控协议未改，无需重打包。

## 真机检查

设备为 HarmonyOS 7 / API 26。覆盖安装与启动成功；首次启动因锁屏被系统拒绝，用户解锁后继续。保留原设备资料，由 Windows 正常批准会话。

- 竖屏 1× 图像边界 `[0,1045][1316,1786]`，垂直中心约 1416，与物理屏幕中心一致，远端完整显示。
- 横屏 1× 图像边界 `[246,0][2585,1316]`，从屏幕顶端显示到底端。
- 通过 hdc 点击浮球，菜单展开前后图像边界均为 `[246,0][2585,1316]`，显示尺寸和位置不变。
- 截图发现原 `borderRadius` 后调用 `border` 会被默认 radius=0 覆盖，最终包改为先 border 再 borderRadius 并 clip；只修改样式。
- 最终样式包再次获用户正常批准后截图确认圆形浮球。展开/关闭菜单、向上拖动浮球后，画面边界仍为 `[246,0][2585,1316]`；浮球从 `[2636,594][2790,748]` 移到 `[2636,417][2790,571]`，松手没有误开菜单。

截图与布局转储只保留在本地忽略目录，不提交包含用户桌面的图片。完整输入体验、键盘覆盖时的画面及用户实际缩放/旋转行为仍按真机验收继续确认，自动化不能替代这些实际操作。

## 官方依据

下列文档已通过 HarmonyOS developer knowledge MCP 检索并读取：

- [安全区域](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-attributes-expand-safe-area)：ignoreLayoutSafeArea API 20，结合 LayoutPolicy.matchParent 扩展实际布局；工程最低 API 22 可用。
- [UIContext](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-uicontext-uicontext)：getKeyboardAvoidMode 与 setKeyboardAvoidMode；NONE 表示不调整内容避让键盘。
- [触摸事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-touch)：windowY 从 API 10 可用，vp 坐标；stopPropagation 阻止事件冒泡。
- [点击事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-click)：onClick(callback, distanceThreshold) 从 API 12 可用，超过阈值取消点击识别。
- [边框](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-attributes-border)：统一 border 未传 radius 时，borderRadius 必须后调用才生效。

## 回归面与限制

现有生产文件只改共享 `canvas.rs` 的可选整屏分支、鸿蒙 `SessionInputController.ts` 的配置、`RemoteSession.ets` 的显示/浮层和 `Desk.ets` 根窗口/键盘布局。C ABI 函数与 state JSON 未变，header 仅补配置说明；共享 fixture 调整为新的完整视口中心。新 `SessionToolsModels.ts` 只负责浮球布局和拖动，不管理远端手势。

本轮未改 Windows、旧 direct/ID 网络与认证、Flutter 调用或用户本地签名/SSH 配置。竖屏系统状态栏文字在深色背景对比不足是此前布局已有现象，本轮没有为它增加窗口级状态；可单独调整状态栏样式。全屏后断开的结束页保持会话布局，返回设备页时恢复普通布局。
