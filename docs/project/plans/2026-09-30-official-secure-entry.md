# 官方 Windows 被控端安全入口实现计划

> 使用 subagent-driven-development 分解实现和独立复核；沿用用户指定的 `feat/harmony-controller`，不改 DevEco 本地签名资料。

**目标：** 将严格签名直连和现场批准接入官方 `Connection` / 连接管理器。当前切片只交付非媒体安全准入，视频、系统输入和最终界面独立推进；真机按用户要求跳过，不能记录为通过。

**架构：** 新增 Windows 专用、默认关闭的 Cargo feature `ord-secure-host`。启用该 feature 的构建以独立安全监听替代 rendezvous/旧 direct/LAN 启动，所有 `create_tcp_connection` 都走严格 v1 握手。官方 Connection 的产品分支采用明确的消息白名单、真实 CM 现场批准和全 false 权限快照，不调用既有自动密码授权、OS 登录、2FA 可信标签豁免、屏幕订阅及输入注入。

**本轮取舍：** 普通构建保持原有代码路径；安全构建默认不监听。环境变量 `ORD_SECURE_LISTEN` 指定具体 IP:port，拒绝通配/零端口；非 loopback 需 `ORD_SECURE_ALLOW` 指定最多 16 个来源 IP，空列表不得理解为所有来源。可选 `ORD_SECURE_PROFILE_OUT` 导出 Config 的公开身份，私钥不写入文件或日志。签名缺失、错误消息、空密钥、v0/未知版本、密文错误都关闭。仅支持 TCP；不把已有非严格通道作为兼容回退。

批准模式为此工程阶段明确的“仅现场批准”，不是主 SPEC 尚未完成的短期凭据 + 批准组合。配置了 2FA 则本轮拒绝，避免把点击当作豁免；无无人值守/旧会话继承。重复 LoginRequest 关闭，审批前及后都不允许危险消息。常规客户端 `option`、版本/平台等元数据允许出现但完全不解释，不设置旧 options_in_login，不调用旧选项处理。本机开启键鼠开关仍返回 false（该切片尚无画面/系统输入能力）。成功 PeerInfo 使用 `ord_secure_host=1, media=false, input_scope=none`，绝不宣称 `ord_demo`。

## 文件和任务

- [x] S1：`src/server/secure_host_policy.rs` 的纯策略、身份/握手协议辅助函数；`libs/controller-core/tests/official_host.rs` 直接编译同一源码，用同一上游 protobuf/Encrypt 验证。先写测试并观察失败，再实现监听配置校验、握手严格拒绝、签名/双向密文、登录字段/消息白名单和权限快照。
- [x] S2：新增 `src/server/secure_host.rs` 的异步监听/握手适配；`Cargo.toml` 和 `src/server.rs` / `src/rendezvous_mediator.rs` 仅增加 cfg 薄入口。复用已有 admission limit、timeout、Stream 和 Config 密钥，不重写密码算法。
- [x] S3：新增 `src/server/connection/secure_host.rs` 与 `connection.rs` 薄 hook：初始化关闭权限；消息优先分流；CM 授权/关闭专用处理，丢弃其余危险 IPC；不进入旧 send_logon_response 副作用；保留包大小上限和有限会话寿命。正式端能力与 DEMO 清晰区分。
- [x] S4：提供正式构建/启动说明、构建预检脚本和实际命令日志。运行官方 Windows crate feature off/on 检查；若构建依赖阻塞，记录精确阶段，不能把独立协议测试当作整个 Windows EXE 已构建。
- [x] S5：执行原 53 项与新增策略/协议测试，格式、最小回归范围和独立安全复核。更新 PROGRESS / DECISIONS / ROADMAP，单独提交本轮代码；不提交用户签名配置或构建缓存。

## 具体验收

`cargo test --manifest-path libs/controller-core/Cargo.toml --locked` 保留现有 44 项，新增用实际协议类型验证：无有效身份不发 Hash；错误/空 PublicKey、v0 和未知版本拒绝；v1 签名校验与双向加密成功；无显式监听/来源时不开端口；任何登录扩展、OS 凭据、未知/操作消息不能产生权限。`cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked` 保留 9 项。

正式构建分别执行 `cargo check --locked -p rustdesk --lib --no-default-features` 与 `cargo check --locked -p rustdesk --lib --no-default-features --features ord-secure-host`。功能关闭不改变已有握手/认证/权限路径；功能开启不经过旧明文 direct 和 rendezvous/LAN 启动。最终报告区分：纯策略/协议源码测试、官方集成编译、真实 CM 互通、视频与真机。

## 本轮验收结论

59 项测试、官方 Windows feature-off/on 编译检查及开发版 EXE 链接通过；见 [验证记录](../research/2026-09-30-official-secure-entry-validation.md)。真实 CM GUI 端到端与视频/系统输入未验收，源码/编译范围完成不能代替这些后续验证。
