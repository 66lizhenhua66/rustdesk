# 正式审批窗口与模式绑定验证

日期：2026-10-02，基线 `9e15b1498`，分支 `feat/harmony-controller`。

## 已验证

- Debug CM 从未修改的 `rust-sciter` 提交 `5322f3a` 构建，实际启动后进程保持运行，窗口标题 RustDesk，CM IPC 服务成功建立。未使用缓存补丁或关闭 Debug 检查。
- 崩溃的原因是旧 wrapper 将组合位掩码 transmute 成没有对应变体的 enum。安全构建的 CM 使用合法的单个 SW_MAIN 标志，由既有 HTML 绘制边框；只订阅其实现的脚本回调，attachment 通知无条件保留。普通构建和非 CM 窗口路径不变。
- `expectedPeer` 将持久连接绑定为本机选择的模式：不填默认 DEMO，正式按钮显式传 `secure_host`。错误或混合能力标记拒绝；单次登录接口不接受该字段。批准前后任何 enabled=true 都拒绝，正式模式的 pointer/text API 返回未授权。
- 先运行旧实现观察错模式、批准前权限提升用例失败，再修复通过。最终共享核心 53 项、Windows DEMO 9 项、官方审批状态 1 项，共 **63 项**通过；并非运行全部官方测试。
- 修复后的官方 Debug EXE、双 ABI 鸿蒙 HAP 均构建成功，最终 HAP 已安装到 API22 模拟器。连接后文案改为“Windows 本机已批准”，不再错误地显示仍等待批准。

## 真实本机批准与连接验证

窗口内容读取的 Computer Use 应用授权等待超时，因此由本机用户实际点击 CM 接受。随后读取模拟器界面，确认“正式安全入口已连接 · 只读”，并核对 21120 的 TCP Established 连接仍属于本任务的正式服务进程。没有用 IPC 注入或自动批准测试入口替代本机操作。

用户反馈点击后窗口似乎退出。检查发现这是旧 `src/ui/cm.tis` 接受按钮在 30ms 后自动最小化的行为；当时 Windows 服务和 CM 进程仍存活，模拟器已处于 connected，并非异常退出。未修改该既有窗口行为。

继续通过模拟器执行“断开连接”，确认回到未授权；再次连接时显示“等待正式入口批准”，没有继承上次批准。随后取消新请求，结束本轮测试会话。实际批准、持续只读、客户端断开和重连重新审批已验证；Windows 拒绝按钮与 stop-service 的实际点击尚未单独测试。视频和系统输入仍未开放。

## 构建和证据

命令：`cargo test --manifest-path libs/controller-core/Cargo.toml --locked`、Windows DEMO 对应测试，以及 `scripts/build-official-secure-host.ps1 -TestSecureGate` / `-DebugBuild`、`apps/harmony-controller/scripts/build.ps1`。

本机忽略的 `apps/harmony-controller/artifacts/cm-*.log` 保存红/绿测试、构建和 CM 启动证据，`cm-review.md` 记录独立复核与修复闭环。用户签名配置、IDE 文件、原型和任何 Cargo 缓存变化未纳入提交。

实际界面证据位于忽略的 `cm-approved.json`、`cm-client-disconnected.json`、`cm-reconnect-pending.json`、`cm-reconnect-cancelled.json`；只记录测试会话状态，未提交原始界面数据。

## 回归影响

`libs/controller-core/src/session.rs` 固定所选模式及正式模式权限门禁；对应头文件说明新请求字段。`Index.ets` 仅给正式按钮加明确模式，并修正批准后文案。`src/ui.rs` 和 `src/ui/cm.rs` 仅对 Windows + ord-secure-host 的 CM 采用有效枚举 API，解决阻断审批的崩溃。未改 Windows 服务认证和审批实现、上游加密或协议。普通构建 UI、DEMO 输入和旧单次登录均保留。

Release 未触发非法枚举检查不能证明其行为安全。此前带临时依赖补丁的二进制不作为本次交付或运行验证依据。正式远程画面/系统输入仍未开放。
