# ID、中继与可信设备无人值守实施计划

日期：2026-10-05。基线：`88e2387e0`。用户已确认按「安全 ID/中继 → 可信设备无人值守 → 同步界面、测试与文档」顺序实施。沿用当前 `feat/harmony-controller` 与现有本地构建环境。

**目标：** 鸿蒙通过自建协调服务寻址并直连或中继至正式 Windows 安全入口，随后支持本机明确登记的控制设备在授权范围内无人值守接入。

**架构：** 在独立 controller-core 内增加 rendezvous transport，复用已有 protobuf、BytesCodec 与上游加密实现。Windows 安全入口增加显式配置的注册/中继路径。所有路径继续使用已固定的目标 ID/公钥和现有严格握手，连接成功不授予权限。无人值守使用单独版本化的配对与签名证明，保留现场批准入口。

**技术栈：** Rust、libsodium、上游 rendezvous/message protobuf、ArkTS/NAPI、Flutter、Windows 本机配置与鸿蒙系统安全存储。

## 全局约束

- 不修改 `libs/hbb_common` 子模块，不搬入整套桌面客户端，不更改旧 direct/DEMO 的授权语义。
- ID 是寻址信息。服务端公钥与目标公钥分别验证；不得接受网络返回的新公钥替换已固定身份。
- 仅使用显式配置的自建服务，无公共服务默认值。缺公钥、错签名、目标错配、未知会话协议代际均拒绝；身份错误不触发降级重试。信令采用上游已签名的 v0/v1 密钥协商；Windows 复用上游最高共同版本规则，服务端声明的更高能力不等于协商使用该未知版本。目标会话始终固定 KX1。
- ID 自动路径允许协调 TCP 直连失败后安全中继，relay 模式禁止直连。实际路径来自核心事件，不从选项推断。
- 无人值守不使用 hwid/名称作为身份，不继承一次现场批准、近期会话或旧 trusted devices 的权限。
- 凭据不进入 profile 导出、Preferences、日志、Git 或截图。无法安全持久化时明确失败。
- 配对、无人值守、撤销、过期各有显式状态；每次新连接进行新的持有证明，撤销同时结束有关活动连接。
- 先完成能力再记录结果；回环协议测试、模拟器、真实 hbbs/hbbr、局域网、公网和真机分别标记。

## R1：核心 ID/中继传输

文件：新增 `libs/controller-core/src/rendezvous.rs`、`tests/rendezvous.rs`；修改 `src/session.rs`、`src/lib.rs`、必要的 Cargo 依赖。

- [x] 在现有 C ABI 请求增加可选 `mode`（默认 direct）、`server`、`serverKey`、`relayServer`。ID/relay 的 peerId 是目标 ID；endpoint 仅供 direct 使用。
- [x] 先用本地协议对端写失败测试：协调直连、强制中继、直连失败后中继、错服务器签名/错目标密钥拒绝、取消与超时。
- [x] 复用上游服务 KeyExchange 的签名验证与 Encrypt；收到并验证服务端响应前不发送敏感认证材料。校验服务签名的目标密钥与保存的目标密钥一致。
- [x] 复用 PunchHoleRequest/Response、RelayResponse、RequestRelay；中继返回的 TCP 流继续进入原 SignedId/v1 会话握手。严格限制帧长、等待时间和请求数量。
- [x] 保留 direct 原执行分支；新增事件 `transport_selected`，附带 `connectionPath`（direct/id_direct/relay）。
- [x] 运行 `cargo test --manifest-path libs/controller-core/Cargo.toml --locked`。

## R2：Windows 自建服务接入

文件：新增 `src/server/secure_rendezvous.rs`；薄接线 `src/server/secure_host.rs`、`src/server.rs`；修改正式启动脚本。

- [x] 显式读取 `ORD_SECURE_RENDEZVOUS`、`ORD_SECURE_SERVER_KEY`、`ORD_SECURE_RELAY`；无配置仍使用原受限监听。
- [x] 使用既有安全服务握手与上游协议注册目标 ID/公钥，处理协调 TCP 和中继请求。只允许配置的中继地址，不打开额外全网卡监听。
- [x] 所有取得的连接都调用现有严格 identity_handshake/run_secure_host；服务关闭、未加密、来源不符或协议错误时拒绝。
- [x] 导出不含凭据的 ID profile，提供实际启动参数与失败提示。
- [x] 定向测试配置和路径边界，然后运行 `scripts/build-official-secure-host.ps1 -CheckOnly`。

## R3：鸿蒙连接界面

文件：`Desk.ets`、`DeskModels.ts`、`libcontroller/Index.d.ts` 与对应模型测试。

- [x] 设备表单可选择 IP、设备 ID（自动）、强制中继；分别显示端口或自建服务与验证公钥字段。
- [x] 统一请求构造函数为 mode-specific 请求，仅在必须资料完整时可连接，保留旧 profile 兼容。
- [x] 显示实际连接路径，服务信任失败/目标离线/中继失败均有明确提示；不显示假成功状态。
- [x] 运行鸿蒙模型测试和双 ABI/HAP 构建，随后审查 R1—R3 的规格符合性与回归面。

## U1：可信设备配对与持有证明

文件：新增 feature-specific 授权策略模块与协议消息；核心、NAPI、ArkTS 和 Windows 安全连接中增加薄接线。

- [x] 单独版本化配对记录，绑定目标 ID/密钥、控制端 Ed25519 公钥、随机凭据标识、用途/能力、有效期和授权版本。
- [x] 配对仅在本机明确允许登记的入口发起，现场批准后持久化；控制端不得请求扩大本机允许范围。
- [x] 每次连接以当前加密会话的全新 challenge 签名，绑定目标与请求能力；错误签名、重放、跨目标、过期和撤销测试先失败再实现。
- [x] 首版无人值守只读，独立于控制端键鼠能力开关；启用额外能力另需明确登记。
- [x] 本机可撤销单设备或全部设备，活动会话同步结束；默认 15 分钟无输入、最长 8 小时，不静默恢复旧权限。
- [x] 2FA 有策略时不可用配对或现场批准隐式豁免，缺少支持的流程明确拒绝。

## U2：平台凭据、管理界面和集成验收

- [x] 查阅 HarmonyOS 6.1 官方 HUKS/Asset Store 文档并记录 API 版本、设备支持、权限和存储等级；接入系统安全持久化，不使用普通 Preferences 保存秘密。
- [x] Windows 本机提供配对登记与撤销管理，鸿蒙提供绑定状态和显式无人值守连接选择，安全错误不会自动重试成现场批准。
- [x] 完成核心负向回归、Windows/ArkTS 构建和安全专用测试对端的集成验证。
- [x] 同步 PROGRESS、入口说明、两个 README 和真机待验清单。最终只报告有证据的范围；文件传输仍单独切片，不把文档同步称为文件传输实现。

- [ ] 真实 hbbs/hbbr、Asset Store 设备行为、现场登记/无人值守重连与撤销、真机和公网验证，按独立待验清单执行。当前无已配置测试服务器，不把 fixtures 标作实网通过。

## 进度账本

- 2026-10-05：审计完成，执行顺序获用户确认，实施 R1—R3。
- 2026-10-05：R1—R3、U1—U2 实现和受控验证完成；157 项自动化、Windows Debug、OHOS 双 ABI/HAP 通过，独立审查无未解决项。真实服务器、设备存储和现场流程仍待验。
