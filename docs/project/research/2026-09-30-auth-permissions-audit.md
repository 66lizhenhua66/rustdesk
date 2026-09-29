# 当前 RustDesk 认证、权限与公网接入基线核查

- 日期：2026-09-30。
- 产品源码基线：`a7f2260203befb7e9c70b585219f0f0b5ca57703`。
- 核查时 HEAD：`15679d88ab602ea490dad6e0f48ead72e53b1a6d`；上述基线到 HEAD 的 `src/`、`libs/` 没有代码差异。
- `libs/hbb_common` 子模块：`229b904508364c8997aad0fb5af57effac859f60`，工作树提交与主仓库引用一致。
- 方法：先读取仓库规范，使用 GitNexus `query/context` 定位认证、会话和权限入口，再核对当前源码。下文链接均指向本仓库源码，行号对应上述基线。
- 范围：Windows 被控端相关的连接握手、认证、授权、消息分发，以及关联配置、IPC 和审计钩子。仅静态只读核查；没有启动服务、修改配置、运行攻击验证或执行测试。本文不是完整安全审计，也不把待验证设计边界直接定性为漏洞。
- 状态约定：**现有**指源码可证实的行为；**建议**指本产品后续需求，尚未实现或验收。没有核查独立 `rustdesk-server` 管理服务源码，不能从客户端字段推定服务器拥有完整账户、RBAC 或审计能力。

## 结论

当前仓库已有密码、现场批准、TOTP、权限位、消息范围隔离、失败节流和连接限额，可作为改造基础。公网安全不能仅依赖隐藏控制端按钮、设备 ID 白名单或“用了 RustDesk 所以一定加密”。应重点明确强制身份验证与加密、授权条件组合、被控端权限上限、危险通道默认关闭，以及会话撤销的具体行为。

## 1. 密码与现场批准已有实现，但 Both 不是 AND

**现有证据**：

- [ApproveMode](../../../libs/hbb_common/src/password_security.rs#L17) 定义 `Both`、`Password`、`Click`；[verification_method](../../../libs/hbb_common/src/password_security.rs#L42) 支持临时、永久或两类密码；[temporary_password_length](../../../libs/hbb_common/src/password_security.rs#L53) 默认 6 位，也允许配置 8/10 位。
- [verify_h1](../../../src/server/connection.rs#L2454) 对挑战摘要作常量时间比较；[validate_password](../../../src/server/connection.rs#L2554) 分别处理临时和永久密码。
- [Connection::on_message 登录授权分支](../../../src/server/connection.rs#L2994) 对 `Click` 等条件进入人工批准路径；[密码校验成功分支](../../../src/server/connection.rs#L3037) 可直接调用 `send_logon_response_and_keep_alive`，并非等待密码和人工批准同时满足。

**建议，尚未实现**：

首阶段默认有人值守协助，必须由现场用户批准。若产品还要求先验证临时口令，应实现清晰的“口令通过且现场批准”组合条件，不能用现有 `Both` 名称宣称该要求已满足。无人值守作为单独策略显式启用；设置永久密码不应隐式开启无人值守。明确口令长度、有效期、轮换、失败限制，以及断线恢复是否保留授权。

## 2. TOTP 已有实现，但不是全部路径强制第二因子

**现有证据**：

- [TOTPInfo::new_totp](../../../src/auth_2fa.rs#L29) 创建 TOTP；[TOTP 秘密存储](../../../src/auth_2fa.rs#L55) 和 [get_2fa](../../../src/auth_2fa.rs#L108) 提供配置存取。
- [send_logon_response_and_keep_alive](../../../src/server/connection.rs#L1843) 会在需要时返回 `REQUIRE_2FA`；[Auth2fa 消息处理](../../../src/server/connection.rs#L3045) 检验代码和失败次数。
- [现场 IPC Authorize](../../../src/server/connection.rs#L750) 会先 `require_2fa.take()`，再进入授权；因此现场批准可以代替此路径的 TOTP。
- [is_recent_session](../../../src/server/connection.rs#L2600) 支持近期会话复用；[可信设备分支](../../../src/server/connection.rs#L2799) 比较 `hwid/id/name/platform`、检查记录时效，然后清掉 `require_2fa`；[添加可信设备](../../../src/server/connection.rs#L3070) 在 TOTP 验证后保存记录。

**建议，尚未实现**：

分别定义“现场批准足以完成有人值守协助”和“无人值守需要第二因子”的条件。不能把匹配现有 `hwid` 记录称为硬件密钥证明。若未来提供可信设备免 TOTP，必须研究设备密钥绑定、凭据保管、撤销、失窃处理和恢复会话的失效规则。首阶段可关闭可信设备豁免，减少需证明的授权路径。

## 3. IP 与 ID 白名单已有实现，ID 不是可信身份

**现有证据**：

- [check_whitelist](../../../src/server/connection.rs#L1410) 检查 IP/CIDR。
- [check_id_whitelist](../../../src/server/connection.rs#L1439) 检查 ID 列表，空列表允许所有 ID。
- [ID 检查注释与分支](../../../src/server/connection.rs#L1448) 明确 `lr.my_id` 是 `self-reported`，白名单匹配不能证明对端拥有此身份。

**建议，尚未实现**：

设备 ID 只作寻址标识，ID 白名单只作附加过滤。核心授权必须依据现场确认、密码或经过验证的设备凭据。未来账户/设备 ACL 应绑定可信身份。IP 白名单需说明在中继、移动网络、IPv6 下的实际匹配来源；不可把 IP 白名单视为所有场景稳定可用的身份认证。

## 4. 身份签名和加密存在，但有非安全兼容降级

**现有证据**：

- [Client::secure_connection](../../../src/client.rs#L1643) 从配置取得服务端公钥；[设备公钥验证](../../../src/client.rs#L1665)、[对端签名验证](../../../src/client.rs#L1694) 与 [设置会话密钥](../../../src/client.rs#L1728) 构成安全连接流程。
- [没有可信身份时的分支](../../../src/client.rs#L1680) 发送空消息并返回；代码注释明确描述非安全兼容路径。
- [公钥不匹配的兼容分支](../../../src/client.rs#L1749) 对 TCP 等保留 `fall back to non-secure`。不能将这一行为概括为“所有 RustDesk 连接必然通过对端身份验证”。
- [WebRTC 指纹绑定](../../../src/client.rs#L1703) 在可信身份建立后验证 DTLS 指纹，绑定不一致拒绝连接；缺少可信身份和已有可信身份但绑定失败是不同分支。
- [server::identity_handshake](../../../src/server.rs#L261) 在 `secure` 且密钥有效时进行签名和密钥协商。

**建议，尚未实现**：

公网产品模式必须验证对端身份并确认加密成功，失败即拒绝，不自动进入非安全路径。服务端公钥缺失/错配、设备公钥变化、回退中继、重连和旧版对端均列负向验收。服务端根公钥用于验证绑定关系，不代表中继需要或应该获得终端会话私钥。身份信任模型还要明确是否信任协调服务签发关系，是否要求配对后固定设备公钥。保留成熟密码学原语，不自创算法。

## 5. 被控端有实际权限检查，但当前优先级不是权限交集

**现有证据**：

- [Connection::permission](../../../src/server/connection.rs#L2646) 映射键鼠、剪贴板、文件、声音、摄像头、终端、隧道、重启、录制、阻止本地输入和隐私模式。服务端 `ControlPermissions` 含有对应值时直接返回该值；否则回退 [is_permission_enabled_locally](../../../src/server/connection.rs#L2629)。本机 `full/view` 模式又有自己的取值规则。
- [peer_keyboard_enabled](../../../src/server/connection.rs#L2311) 组合本端与对端开关；Windows 的 [鼠标](../../../src/server/connection.rs#L3159)、[触控](../../../src/server/connection.rs#L3222)、[键盘](../../../src/server/connection.rs#L3290) 消息均在执行前检查。
- [文本剪贴板](../../../src/server/connection.rs#L3347) 和 [重启](../../../src/server/connection.rs#L3888) 有独立门禁；[SwitchPermission](../../../src/server/connection.rs#L798) 允许现场更新会话权限并通知对端。

**建议，尚未实现**：

建立明确的产品授权规则：有效权限是“本机安全上限 ∩ 已验证身份授权 ∩ 本次现场批准 ∩ 请求范围”。这是拟新增要求，当前实现不能直接视为已满足。默认拒绝未声明能力；权限由被控端最终执行，控制端仅展示能力和请求。区分可用能力、用户请求、已经获准权限；控制端自报支持某功能不等于获准使用。

## 6. 独立会话和旁路能力必须共同纳入授权

**现有证据**：

- 登录时分别检查 [文件会话](../../../src/server/connection.rs#L2877)、[摄像头](../../../src/server/connection.rs#L2889)、[终端](../../../src/server/connection.rs#L2898)、[隧道](../../../src/server/connection.rs#L2919) 权限。
- [authorized_scope_violation 调用点](../../../src/server/connection.rs#L2857) 和 [authorized_message_scope_violation](../../../src/server/connection.rs#L5847) 区分已认证会话能处理的消息范围。
- [FileAction](../../../src/server/connection.rs#L3462)、[FileResponse](../../../src/server/connection.rs#L3799) 及 [Windows 剪贴板文件 IPC](../../../src/ui_cm_interface.rs#L629) 是不同路径，后者还检查文件剪贴板启用状态和授权状态。
- [handle_port_forward_channel](../../../src/server/connection.rs#L2371) 在复用隧道帧处理时检查权限；[handle_terminal_action](../../../src/server/connection.rs#L6222) 要求有效 OS 用户 token。

**建议，尚未实现**：

首阶段关闭文件、文件剪贴板、终端、TCP 隧道/RDP 转发、远程打印、摄像头、隐私模式和阻止本地输入等非必要入口，关闭必须落实在被控端，不能只删鸿蒙按钮。以后文件权限细分上传/下载/删除及目录范围。撤销权限应使已建立任务、文件读写、子通道按策略停止，不能仅影响下次登录。以恶意或修改后的客户端发送被禁消息进行负向验收；分散的现有检查不能直接证明所有旁路均已覆盖。

## 7. 管理员提权目前复用键鼠权限

**现有证据**：

- [ElevationRequest](../../../src/server/connection.rs#L3903) 将请求交给 [handle_elevation_request](../../../src/server/connection.rs#L4619)。后者在 `self.keyboard` 为 false 时拒绝，否则在相应便携模式下尝试启动高权限服务。

**建议，尚未实现**：

新增独立 `elevate_admin` 能力，默认关闭并要求明确现场批准。普通键鼠控制权限不能隐式等于管理员提权权限。UAC、安全桌面、锁屏/登录界面、安装系统服务分别验收，不能以某一个路径成功宣称完整支持。操作系统自身权限提示仍须遵守，不设计绕过系统安全限制的路径。

## 8. 已有失败节流、未授权连接限额与密码轮换

**现有证据**：

- [LOGIN_GRACE](../../../src/server/connection.rs#L87) 为 180 秒；[MAX_UNAUTHORIZED_CONNS](../../../src/server/connection.rs#L90) 为 64；单地址上限在 [MAX_UNAUTHORIZED_CONNS_PER_ADDR](../../../src/server/connection.rs#L95) 为 16。
- [admit_unauthorized](../../../src/server/connection.rs#L123) 在身份握手前分配未授权连接名额；[check_failure_with_scope](../../../src/server/connection.rs#L4437) 处理失败策略，包含 IPv4 地址与 IPv6 前缀。
- [check_update_temporary_password](../../../src/server/connection.rs#L2507) 在连续临时密码错误后轮换；[成功授权](../../../src/server/connection.rs#L1891) 释放未授权名额并更新临时密码。

**建议，尚未实现或验收**：

优先复用而非另写一套重复节流。测试中继来源地址可信性、分布式尝试、资源上限、连接提示轰炸和恢复会话。产品策略需要明确最大会话时长、空闲断开、现场一键断开、锁屏/挂起行为，以及撤权生效时间。现有节流不等于公网服务已经通过抗攻击验收。

## 9. 固定 IP 直连入口不同于经过协调的 P2P 打洞

**现有证据**：

- [direct_server](../../../src/rendezvous_mediator.rs#L1354) 受配置控制，启用后通过 `listen_any` 监听。
- [创建直接 IP 连接](../../../src/rendezvous_mediator.rs#L1403) 传递 `secure=false` 与 `ConnectionMeta::default()`；`:1408` 注释明确直接连接没有服务端用户上下文。

**建议，尚未实现**：

首阶段默认关闭固定 IP 接入和无必要的局域网发现，不要求终端进行公网端口映射。未来局域网/EasyTier 接入需要显式启用并应用同样的身份、加密与授权门禁，避免绕过账户策略。关闭该固定直连监听不等于关闭服务器协调的 P2P 打洞；仍可保留正常的直连优先、中继兜底路线。

## 10. 有审计钩子，不等于已有完整账户和管理服务

**现有证据**：

- [get_api_server](../../../src/server/connection.rs#L1525) 获取审计地址；[post_conn_audit](../../../src/server/connection.rs#L1550) 提交连接记录，地址为空时返回；[post_alarm_audit](../../../src/server/connection.rs#L1618) 提交告警。
- [get_audit_server](../../../src/common.rs#L1224) 组装 `/api/audit/...` 路径。
- [connection_meta](../../../src/rendezvous_mediator.rs#L42) 可传递服务端权限与上下文，但本仓库客户端代码并不能证明 OSS `hbbs/hbbr` 提供完整账户、RBAC、审计存储服务。

**建议，尚未实现**：

首阶段先提供可靠的本地持久审计，记录设备、身份验证状态、授权来源、权限变化、连接路径、时间和结果，不记录密码、令牌、私钥或剪贴板正文。文件名、来源 IP 等审计数据也需保存期限与访问控制。将来多人管理需要单独选择或实现账户/设备策略服务，支持短期授权、撤销和集中审计；`hbbs/hbbr` 继续负责协调/中继，不能默认承载所有产品授权职责。

## 可用于后续 SPEC 的验收边界

以下是建议测试方向，不代表测试已经执行或产品已经通过：

| 场景 | 预期结果 |
|---|---|
| 无身份验证或密钥错配后请求连接 | 拒绝；不自动变为未认证/未加密会话 |
| 正确临时口令但本次授权要求现场确认 | 等待确认；批准前不输出受保护画面、不执行输入 |
| 只读权限下手工发送键鼠、提权、终端或文件消息 | 被控端拒绝，不因控制端 UI 或协议消息类型改变而放行 |
| 冒用允许列表中的设备 ID | 仍须完成真正认证，不能仅靠 ID 登录 |
| 现场撤销键鼠或断开会话 | 在规定时限内停止执行和清理状态，避免按键残留和未授权自动恢复 |
| 非法认证请求、并发握手、IPv6 地址轮换 | 有界占用资源并按策略限速，正常授权流程仍可恢复 |
| 从 P2P 回退到中继 | 保持同等身份、加密与授权要求 |
| 未来启用可信设备、无人值守或 IP 直连 | 单独完成密钥绑定、失效与旁路验证后才启用 |

本文件用于保留源码证据和需求边界，不替代发布前代码审查、依赖安全更新、协议负向测试或真实网络验证。
