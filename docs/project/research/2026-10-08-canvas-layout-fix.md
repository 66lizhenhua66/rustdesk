# 真机画布可用面积与底部白条修复

日期：2026-10-08。基线：`6a0792352`。用户要求通过 hdc 截图检查画布过小，并取消底部留白、将控件上提。

## 截图与原因

HarmonyOS 7 / API 26 真机横屏截图为 2832×1316。原折叠工具栏状态下，画布可见边界为 `[204,252][2748,938]`，只有 686 像素高；下方常驻说明条占据 `[162,1043][2433,1176]`，系统底部区域另呈白色。

问题不是远端分辨率或缩放坐标错误。页面给顶部工具栏预留 48 vp，底部说明条再预留 56 vp，核心配置又在上下各加入 24 vp 留白。三层预留共同压缩了用于 fit 和裁剪的安全画布。

## 调整

- 会话默认使用收起的顶部悬浮条，显示缩放倍数和控制状态；展开后仍可操作键盘、工具、返回及断开。
- 还原按钮移到顶部，展开和收起时都可用。取消底部常驻说明条及其 56 vp 高度预留；错误和长按拖动保留短暂悬浮提示。
- 鸿蒙画布额外间距从 24 vp 减为 8 vp；系统安全区仍按实际重叠计算，图片布局、裁剪和输入继续共用同一变换。
- Desk 根背景改用 API 20 支持的 `background(ResourceColor)`，会话时深色、设备页浅色。背景可延伸到父安全区，内容布局保持原避让，未修改全局导航条或系统窗口设置。

已通过 HarmonyOS developer knowledge MCP 检索并读取 [多设备窗口沉浸适配](https://developer.huawei.com/consumer/cn/doc/best-practices/bpta-multi-device-window-immersive) 和 [开发应用沉浸式效果](https://developer.huawei.com/consumer/cn/doc/harmonyos-faqs/faqs-arkui-1089)。背景绘制延伸不会要求应用把点击区域放到系统手势区内。

## 验证

- 鸿蒙相关 Node 回归 30 项通过。
- 更新共享配置 fixture 中的 padding，Rust `shared_canvas_event_fixture_matches_public_abi` 通过，中心点击仍得到相同远端坐标。
- Hvigor HAP 构建成功，既有签名身份的签名、原生 codesign 和 profile 验证通过。最终包 SHA-256：`e615d4be7bb0c5360d8169c1172fffba7feb2421e4b7766f476b1f9aff828f7c`。
- 真机覆盖安装、启动成功。用户重新进入会话后再次通过 hdc 截图，底部白条消失，背景与系统手势区域连成深色；底部整条说明已消失，顶部还原按钮可见。
- 新截图在工具栏展开时画布边界为 `[148,291][2804,1190]`，高度 899 像素，比原折叠状态仍增加 213 像素。按同设备几何计算，折叠状态可达约 994 像素；该数为计算值，不冒充本轮截图实测。

原截图与更新截图分别保存于忽略的 artifacts 目录下 `real-device-canvas-size.png` 和 `real-device-canvas-layout-after.png`，不提交包含桌面内容的截图。独立审查未发现阻塞项。

## 回归面

仅改变鸿蒙 `RemoteSession.ets` 的工具栏和提示布局、`SessionInputController.ts` 发出的画布 padding、`Desk.ets` 根背景及对应 fixture。共享 Rust、原生渲染、触摸手势、输入授权与 Windows 被控端保持原实现；Windows 无需重打包。屏幕宽高比造成的正常居中黑边仍保留。
