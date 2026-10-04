# 控制端能力开关现场验证

日期：2026-10-04。实现提交基线 `499b89441` 之后，按 [控制端能力开关计划](../plans/2026-10-04-controller-capabilities.md) 做了手机 API22 模拟器与 Windows Flutter Debug 工作台联调。该记录只报告实际看到的状态。

## 已验证

- 新版手机请求到达 Windows Flutter 工作台后，用户只点击一次“批准连接”。没有第二次 Windows“允许键鼠”步骤。
- 手机初始保持“仅查看”，显示“点‘控制’开始操作”。点击手机“控制”后，状态变为“控制中 · 键鼠已开启”，出现触控板、鼠标按钮、滚轮按钮和键盘入口。
- 从手机键盘入口输入并确认 `REMOTE-中文-2`，Windows 前台专用记事本显示为 `REMOTE-中文-2Open Remote Desk input validation`；说明确认文本经过正式控制链路到达 Windows。输入文件位于忽略目录 `apps/harmony-controller/artifacts/input-validation-20261004.txt`。
- 手机点击“仅查看”后，状态回到“仅查看 · 点‘控制’开始操作”，输入面板停止，视频会话仍保持。控制端关闭流程实际回执通过。
- 本轮没有单独宣称鼠标移动、左右/中键、滚轮、物理按键或拖动已经通过；它们已有模拟注入和模型回归，现场验证留待下一次在同一安全测试文件上补齐。

这次现场流程使用新版 `input_version=2`：Windows 只显示和执行一次接入批准，控制端在获准后发送能力开关请求。现场没有出现第二次“允许键鼠”提示。

## 自动化与构建

控制端能力调整后，自动化总数为 **122 项通过**：controller-core 84、独立 DEMO 9、Windows 15、Flutter widget 4、鸿蒙模型 10。最终 Windows Debug 整包、鸿蒙双 ABI/HAP 和 `scripts/build-official-secure-host.ps1 -CheckOnly` 均通过；旧只读/DEMO 路径仍有回归。

新流程的协议使用 `input_version=2`、`OrdInputRequest` 和带 `request_id` 的 `OrdInputState`。关闭控制会先清本地令牌和队列，再发送关闭请求；迟到的授权状态和旧心跳不会重新开启控制或断开视频。音频、文件、剪贴板、无人值守和提权保持禁用。

构建时曾遇到旧托盘进程占用 Flutter EXE，确认路径后停止并重试成功；没有停止其他应用。最终 HAP SHA256 为 `72363D29902466F8A528639CAD3E61EDD87D98F743F64E3BCE19AF4C0CBDADA2`，EXE 为 `CD707F4D8851C6F841175E406D0A875E1BFE8A637B0E0D53AB1CCAB9A238444E`，DLL 为 `AA7C412614B75D49B492EC43BE17C985FA07A0D7C362402ACFA7E6E231D7E1E1`。新版 HAP 为未签名模拟器包，不能外推为真机结果。

## 未验收

真机项目统一追踪于 [鸿蒙真机待验清单](../plans/2026-10-04-harmony-device-validation.md)，当前全部待验证；本报告中的手机结果均来自模拟器。后续平台变化须补充清单，不能把尚未实现的声音、硬解或 ID/中继标成仅待真机复测。

真实鼠标和滚轮、基本物理按键、撤销时按住状态释放、断网/半断开、释放失败提示、真机、公网、UAC/安全桌面和系统声音均未完成现场验收。默认只读、`-Input` 本机实现门禁、每次连接一次本机批准和执行点令牌检查仍保留。

原型、用户签名、`.clangd`、`.clang-tidy` 未修改；截图和测试文件不进入 Git。下一步先补齐鼠标/滚轮/基本按键和撤销释放现场验证，再评估声音等独立能力。
