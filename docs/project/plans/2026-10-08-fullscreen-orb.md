# 整屏画布与侧边浮球实施计划

日期：2026-10-08。用户已审阅交互原型并选择「侧边浮球」。本计划替代将显示区域裁成 SafeRect 的旧连接页面布局，继续使用已确认的触控方案。

## 确认的行为

- 整个屏幕作为画布，工具、还原按钮、状态提示和键盘浮在画面上，不为工具栏预留固定区域，也不因展开工具而重算 fit。
- 1× 表示完整显示远端，保持原始宽高比并居中。16:9 远端在更宽的手机横屏中会有左右黑边；自然比例黑边属于画布。缩放范围 1～10×，不会拉伸远端。
- 右侧 44 vp 浮球可上下拖动，受真实系统安全区域约束；点击向屏幕内展开工具。提供控制/仅查看、键盘、触屏/指针、还原、断开与返回设备。点击菜单外关闭只消费当前操作，不点穿远端。
- 放大后浮动还原按钮恢复 1× 和整屏中心；单指平移、1 秒长按远端拖动及双指捏合/滚轮/右击保持现有语义。
- 旋转尽量保持 zoom 与远端中心。额外平移边界允许将远端边缘移入系统安全区域，图片外的黑边不发送 direct 模式点击。
- 系统安全区约束输入和悬浮控件，不缩小整屏画面。会话关闭/后台恢复工作台布局；保留前台会话常亮。

## 文件与步骤

- [x] `libs/controller-core/src/canvas.rs` 和 `tests/canvas.rs`：新增可选 `fullViewport`，默认 false 保留旧调用。true 时 fit/center 使用 viewport，safeRect 仅用于输入和边缘可达；公开 ABI 回归覆盖比例黑边、系统避让不缩图、旋转/reset、平移及黑边点击。
- [x] `SessionInputController.ts` 和共享 `canvas-events.json`：启用 fullViewport，padding=0，发送真实系统 insets；fixture 中心点按继续映射到远端中心。
- [x] `RemoteSession.ets` 与必要局部浮球组件：使用完整 viewport 绘制/裁剪，浮层独立；浮球拖动和菜单关闭不经过远端输入。删除本次替代的旧工具栏占位逻辑。
- [x] `Desk.ets` 与会话窗口适配：通过当前 SDK 支持的组件/窗口 API 达到真正全屏内容、键盘覆盖，退出恢复。先检索并读取官方 Harmony 文档。
- [x] 运行 core 与鸿蒙相关自动化、OHOS 双 ABI/HAP 构建；最小化审查与独立审查；使用既有调试签名并覆盖安装真机，hdc 截图确认横屏完整 1316 像素高度，浮球菜单展开前后画面边界相同。
- [x] 更新 SPEC-005 和进度，记录已验证范围及尚待用户验收的输入体验。仅提交本轮文件，不改用户签名、SSH、本机脚本等配置。

## 验证命令

```powershell
cargo test --manifest-path libs/controller-core/Cargo.toml --locked --offline
node --experimental-strip-types --test apps/harmony-controller/tests/*.test.ts
& 'C:\Program Files\PowerShell\7\pwsh.exe' -NoProfile -File apps/harmony-controller/scripts/build.ps1 -DevEcoRoot 'G:\Huawei\DevEco Studio'
```

截图需区分显示图像与画布边界：图像保持比例可能存在自然黑边，但窗口全屏尺寸不得因菜单开关而变化。构建、签名、安装和真实远端操作分别记录，不把 fixture 当作真机验收。

结果见 [本轮交付记录](../research/2026-10-08-fullscreen-orb-validation.md)。键盘覆盖与完整触控体验继续由真机验收，末次圆角样式修正后已重装，经 Windows 正常批准后复验圆形、菜单开关与拖动位置。
