# 真机触控手势未结束修复

日期：2026-10-08。用户报告：再次缩放、点击或移动画布时，画面像从初始比例重新开始。设备为 HarmonyOS 7 / API 26，无线调试真机；本轮不涉及 hbbs/hbbr。

## 根因与修复

通过临时签名诊断包采集画布事件，确认问题位于鸿蒙触点到共享输入事件的转换。

ArkUI 的 `TouchType.Up` 中，`touches` 仍可能包含刚抬起的手指，`changedTouches` 指出这次抬起的 ID。共享引擎的 `points` 契约则要求只包含仍按下的触点。原适配器直接传递 `touches`，最后一根手指抬起后仍报告一个触点，捏合状态没有结束。下一轮双指按下被当作前一轮继续，使用上次结束距离计算缩放，从约 3.1054× 跳为 1×；旧捏合也吞掉随后的单指画布平移和点击。

真机日志的关键序列（21:59）：

| 事件 | points ID | changed ID | 结果 |
| --- | --- | --- | --- |
| 第一指抬起 | 0、1 | 1 | 仍处于 pinch |
| 最后一指抬起 | 0 | 0 | 3.1054×，仍处于 pinch |
| 下一轮第一指按下 | 0 | 0 | 仍沿用旧 pinch |
| 下一轮第二指按下 | 0、1 | 1 | 倍数从 3.1054× 变为 1× |

修复只在 `HarmonyInputAdapter` 调用的 `activeHarmonyTouches` 中进行：Up 从 touches 按 ID 排除 changedTouches，Cancel 清空活动触点；保留 changedTouches 的最终坐标用于点击与释放。Down/Move 不变，无新增手势状态。共享 Rust、NAPI、Surface、Flutter 与 Windows 被控端均不需修改。

已通过官方知识 MCP 检索并读取 [触摸事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-universal-events-touch) 和 [支持触屏输入事件](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-interaction-development-guide-touch-screen)。官方示例以 `TouchType.Up && touches.length === 1` 表示所有手指已抬起；触点按 ID 区分，不能依赖数组顺序。真机日志进一步确认了当前设备实际行为。

## 验证与交付

- 新增 2 项触点回归，覆盖双指逐根抬起后 ID 复用、单指点击/拖动结束及取消；先运行失败，再实现通过。
- 真实共享 DLL 回放旧/新事件序列：旧数据在第二次捏合按下时从 2× 降为 1×；修正后的事件保持 2× 再放大到 4×。两次独立平移各累加 20，点击保持 4× 和画布位置，并恰好产生 move、button-down、move、button-up。
- 鸿蒙模型与输入、常亮回归合计 30 项通过；Hvigor `assembleHap` 成功。原生核心未变化，未重复构建 Windows 或双 ABI Rust。
- 最终 HAP 使用现有调试身份签名，`verify-app`、原生 codesign 和 `verify-profile` 全通过，当前设备 UDID 在描述文件中。22:08 签名产物 SHA-256：`8f71a6306ef47c01152e1606b94628d8e661985320be10ce099698b775337913`。
- 真机覆盖安装返回 `install bundle successfully`，启动返回 `start ability successfully`。已保留设备配置，供用户继续验收连续缩放、平移、点击和常亮。

本轮另按用户要求加入 [前台远程会话常亮](2026-10-08-session-keep-screen-on.md)，断开、隐藏或退后台时释放。实际静置超过设备熄屏时间的行为仍需真机验收，不能由模型测试替代。

诊断日志源码已移除，最终包不记录逐触控日志。日志、回放脚本和签名包仅保存在忽略的 `apps/harmony-controller/artifacts/`。未更改用户本地签名配置，也未自动批准 Windows 连接。

## 回归面

既有路径仅改动鸿蒙 `HarmonyInputAdapter.ets` 的触点转换，以及 `RemoteSession.ets` 的常亮生命周期通知。新增转换函数无状态、与其他平台输入契约隔离；常亮通过独立服务串行处理异步原生调用。独立审查未发现阻塞项；Windows 被控端仍使用上一轮已构建的配套版本。
