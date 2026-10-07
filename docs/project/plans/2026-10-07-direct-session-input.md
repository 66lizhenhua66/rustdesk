# 远程画面直接控制

日期：2026-10-07。用户已确认：进入远程分为控制和仅查看，默认控制；移除独立控制面板，键鼠直接映射到被控端。

## 交互与边界

- 普通连接默认选择控制，也可在连接前选择仅查看。收到 Windows 本次连接批准、首帧和输入能力后自动请求启用，收到真实启用回执才转发输入。
- 会话工具栏保留控制／仅查看切换。收起工具栏不关闭控制；切到仅查看、离开会话、后台或断开时释放输入。
- 在保持比例的远程画面上直接点按、拖动；坐标按实际画面区域映射，黑边不产生点击。外接鼠标移动、左右／中键、滚轮与物理键盘走现有输入协议。
- 键盘按钮调起系统输入法，确认的文字直接转发，不再要求点击发送。预编辑候选不得提前发送；关闭键盘不会切换成仅查看。
- 可信设备登记与无人值守会话仍只读。Windows 本机批准、输入能力门禁与执行点授权继续生效。

## 实施与验证

- [x] `InputModels.ts` 与 `input-models.test.ts`：先验证画面位置归一化、默认启用一次、明确仅查看及拒绝后不重试；再实现模型。
- [x] `RemoteSession.ets`：将触摸、鼠标、滚轮和键盘绑定到现有 Surface；移除独立输入面板；接入自动启用和系统输入法。
- [x] `Desk.ets`：连接前提供控制／仅查看选择，将本次选择传给会话，更新相关说明。
- [x] 核对并按需补齐正式输入协议的常见物理键映射，局限在 secure input 路径。
- [x] 运行现有鸿蒙模型测试，构建 HAP；若涉及核心/Windows 键码，运行对应回归。
- [x] 独立审查最终 diff 的默认控制、坐标、重复事件、仅查看和释放边界，更新进度与真机验收项。

验证命令：

```powershell
node --experimental-strip-types --test apps/harmony-controller/tests/desk-models.test.ts apps/harmony-controller/tests/input-models.test.ts apps/harmony-controller/tests/access-models.test.ts
& 'G:\Huawei\DevEco Studio\tools\node\node.exe' 'G:\Huawei\DevEco Studio\tools\hvigor\bin\hvigorw.js' --mode module -p module=entry@default -p product=default assembleHap --no-daemon
```

HAP 命令在 `apps/harmony-controller` 下执行，使用 DevEco 内置 SDK/JBR。保留本地 `build-profile.json5`、构建脚本和 clang 配置。本轮自动化及编译不替代鸿蒙真机输入验收。
