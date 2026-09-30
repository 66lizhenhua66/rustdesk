# 非视频持续会话 DEMO 验证

日期：2026-09-30。分支：`feat/harmony-controller`。基线：`5e6c24d5c`。用户要求先实施可运行 DEMO，视频解码及真机继续后移。

## 实现范围

- 新增独立 `apps/windows-demo-host`：Win32 窗口、临时签名身份、具体 IP 监听、签名加密握手、现场批准、独立键鼠权限、受限画布/文本、撤权和断开。执行点在持锁时验证当前会话与权限，不调用系统 `SendInput`。
- `controller_connection_create` 新入口复用严格身份/加密实现，要求 demo_window 能力标记，默认只读。旧 `controller_session_create` 仍为单次登录、确认后关闭。
- 鸿蒙 ArkUI 导入公开资料时先验证/预览，再显式保存；新增持续 DEMO、配对码、受限操作区和文字发送。NAPI 在后台运行网络任务，UI 取消不等待线程退出，旧任务回调隔离。
- 新增打包脚本及 [DEMO 操作说明](../DEMO.md)。导出资料只含公开身份；不打包固定密码、私钥或签名材料。

## 验证环境与证据

Windows 主机 Rust/Cargo 1.96.1，DevEco SDK 6.1.1.125/API24；HAP 同时包含 ARM64/x86_64 原生库，兼容 API22。运行验证使用已启动的 x86_64 OpenHarmony 6.0.2.130/API22 模拟器，经 HDC reverse port 连接 Windows loopback；**不是鸿蒙 6.1 真机，也不是公网测试**。

本机忽略的 `apps/harmony-controller/artifacts/` 保存测试日志、构建日志、UI 布局与联调脚本。此报告只保留脱敏结论，不提交公开资料快照、截图、二进制或原始日志。

## 测试与构建

共享核心 `cargo test --manifest-path libs/controller-core/Cargo.toml --locked`：44 项通过，原 38 项保留。新增覆盖 DEMO 参数、配对码、缺少能力标记拒绝、键盘许可/加密输入/反馈、取消禁用输入、持续入站消息不饿死输出。最后一项先在旧逻辑下失败，再通过修复验证。

Windows 宿主 `cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked`：9 项通过（4 个执行点/撤权竞态/标签清理/写关闭单测，5 个真实 TCP + controller C ABI 集成测试）。覆盖批准只读、授予后输入、撤权、拒绝/超时、过期批准、新连接重置、错误公钥和通配监听拒绝。合计 **53 项测试通过**，两 crate 的 `cargo fmt --check` 通过。

最终 `apps/windows-demo-host/scripts/build.ps1` 与 `apps/harmony-controller/scripts/build.ps1` 均成功。HAP 含 ARM64/x86_64 的 `libcontroller.so`，未配置签名是已知构建提示；上游 BytesCodec 的未使用方法警告没有新增功能影响。打包产物和本次构建输入的 SHA256 逐一相同：

- EXE：`151e47daa244c2095402cecd91536a46f87a56ed5d9033ed858b72dee738f188`
- HAP：`0854bf4861682906ff1b61c2fa95b2f3beda8c7f27495c0167b9d8e6a74c6c3a`

## 模拟器到 Windows 进程联调

1. 安装 HAP，粘贴本次公开 JSON，验证预览后保存；没有自动连接。
2. 从模拟器主动连接，Windows 为 Pending，双端显示相同六位配对码。
3. Windows Allow connection 后，手机显示只读、文字输入禁用，宿主 input=off。
4. Windows Allow input 后，手机触摸更新宿主坐标；发送 9 字符测试文本，双方显示 `(401,225)` 与文本长度 `9`。此处只是受限窗口操作，不是视频或桌面输入。
5. Windows Revoke input 后，手机显示撤权/只读、文本输入和发送按钮禁用；再次触摸，宿主坐标与文本长度保持不变。
6. Windows Disconnect 后，手机返回未授权状态、移除操作区。

以上流程在最终 HAP/EXE 上再次执行通过，同时确认修复后的窗口标题、独立配对码和连接/断线文案。测试完成后删除本轮资料、关闭测试宿主并移除本次 HDC 转发。没有修改用户其他设备资料或防火墙配置。

## 独立复核与修复

控制端复核指出：持续入站消息可能饿死输出；NAPI 有界队列满时会丢弃权限/断开事件。已分别改为每轮处理入站后检查发送、DEMO 事件队列满时释放取消锁并重试。取消仍可立即标记关闭；旧单次 API 保留原分支。修复经独立复核确认。

宿主复核指出慢写断开延迟、未处理的自报标签干扰配对码显示、累计文本显示不全；已限制单次发送总时间为 2 秒并检测本地关闭、清理标签控制/方向字符且单独显示配对码、添加完整已收文本的滚动控件和画布尾部换行。实际运行还修复了未调用默认 WM_NCCREATE 处理而导致的空标题。上述修复经宿主复核确认；最终跨模块复核没有剩余可操作问题。原始审查报告保留在本机 artifacts，不作为公网安全审计结论。

## 回归影响范围

修改的已有运行路径仅限新增控制端工程：`session.rs` 添加 DEMO 标志、输入队列与持续循环，共用创建校验及握手时按模式选择选项；旧模式保留 30 秒上限、禁用输入和单次关闭。`session.h` 只增加 ABI；NAPI 增加独立 DEMO 启动/输入方法和其事件投递分支；ArkUI 增加导入/DEMO 和取消时清理新状态。现有基础/单次登录测试保留。

上游 `src/`、根 Cargo workspace、原 Windows 主程序、原 `libs/hbb_common` 子模块以及既有帧/加密实现均未修改。其余既有文件变化为 README/决策/进度与索引忽略规则。

## 尚未验证或实现

鸿蒙 6.1 ARM64 真机安装/签名、真实 LAN、防火墙兼容、公网及恶劣网络验收、视频解码/显示、省流与性能、官方 Windows 桌面入口改造、ID/中继、可信控制设备、2FA 继续会话、无人值守、Android/iOS 均未由本轮验收。临时身份每次启动更换；文本/坐标只在当前进程内存。源码测试、模拟器通过和双 ABI 编译不替代这些后续验收。
