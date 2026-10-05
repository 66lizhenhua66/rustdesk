# 可信控制设备只读访问 v1

日期：2026-10-05。属于用户已批准的 ID/中继之后的无人值守切片。适用于本 fork 严格 Windows 安全入口与鸿蒙控制端，不改变上游 hwid trusted devices。

## 用户行为

鸿蒙在设备操作中选择「登记只读访问」，用系统安全存储中的 Ed25519 种子证明设备身份。Windows 明确显示登记请求与公钥指纹，用户选择「批准并登记只读访问」才建立长期授权；普通批准按钮不能登记设备。绑定成功后鸿蒙保存服务端颁发的凭据，显式选择「无人值守查看」即可发起全新验证。

首次登记仍可拒绝、超时、取消。初版配对和无人值守仅协商 `secure_video`，不请求输入；现有有人值守 `secure_control` 保留。Windows 开启无人值守服务必须显式传入 `-Unattended`，现有启动默认关闭。

Windows 管理页列出已登记设备、公钥指纹、到期时间，提供撤销单项和全部撤销。撤销须成功落盘才显示成功，有关活动会话下一次状态检查（最多 1 秒）结束。鸿蒙可删除本机凭据，这不替代 Windows 撤销。

## 协议契约

`LoginRequest` 新增字段 19 `OrdAccessRequest ord_access`。未填写仍执行原有人值守流程。结构为：

```proto
message OrdAccessRequest {
  uint32 version = 1; // 1
  string mode = 2; // pair or unattended
  bytes controller_public_key = 3; // Ed25519, 32 bytes
  bytes credential_id = 4; // pair: empty; unattended: 16 random bytes
  bytes credential_secret = 5; // pair: empty; unattended: 32 random bytes
  bytes proof = 6; // Ed25519 detached signature, 64 bytes
  string scope = 7; // windows_primary_view
}
message OrdAccessGrant {
  uint32 version = 1;
  bytes credential_id = 2;
  bytes credential_secret = 3;
  bytes controller_public_key = 4;
  int64 expires_at_ms = 5;
  string scope = 6;
}
```

`Message` 字段 37 为 `OrdAccessGrant ord_access_grant`。仅配对批准且记录成功落盘后，在加密通道发送一次 grant，随后返回现有 PeerInfo。无人值守成功不重发凭据。PeerInfo 的 platform_additions 添加 `access_mode: pair|unattended`，由核心验证后投影到界面。

签名材料由共享 `libs/base/src/access_proof.rs` 构造，controller-core 按路径引用同一文件。使用 libsodium Ed25519，域分隔 `Open Remote Desk access v1\0`，顺序连接长度前缀字段：目标 ID、目标长期公钥、当前连接 challenge、请求版本/模式/控制设备公钥/凭据 ID/凭据 secret/权限范围。所有可变字段均加 32-bit 大端长度。签名字段本身不参与。拒绝未知字段/版本、非预期长度及范围。

严格入口在发送 Hash 前，把本连接 challenge 设为 CSPRNG 32 字节的 Base64 编码（原 6 字符 challenge 不用于本增量）。请求签名使用本连接实际 challenge 和固定目标身份，防止跨连接、跨目标和跨权限重放。配对也要求持有证明。每连接只允许一次登录，重发不更换凭据。

## 持久化与有效期

Windows 记录包含随机凭据 ID、secret 的 SHA-256、控制设备公钥、显示名称、scope、创建/到期时间、目标 ID/公钥、当前安全策略摘要。secret 明文只在生成和本次加密传输期间存在。每条记录由目标长期签名密钥签名；读取校验完整性、目标绑定和策略摘要。摘要覆盖当前有效密码存储/盐与 2FA 配置，密码、目标密钥或必要因子策略变化即失效。

默认授权 30 天，最多 64 项；默认会话 15 分钟无输入终止，绝对上限 8 小时，并不得越过凭据到期时间。首版只读会话不以心跳续期。2FA 启用时当前未实现继续流程，配对与无人值守均明确拒绝，不允许现场批准绕过。

存储在本机应用配置旁的独立文件，跨进程写入须互斥并原子替换；损坏或保存失败明确拒绝。配对与撤销使用相同存储路径。管理入口只在专用本机 CM 提供，不接受远端协议管理命令。

鸿蒙用 Asset Store（API 11+，本项目 API22/24 可编译）保存 32 字节 seed 和每个目标的凭据；使用 app-private、FIRST_UNLOCKED、AUTH_NONE、SYNC_NEVER，卸载后删除。seed 与 secret 不进 Preferences/profile/日志。存储项按目标 ID 和固定公钥绑定；身份变化不可复用旧凭据。详细官方依据见 [安全存储调研](../research/2026-10-05-harmony-credential-storage.md)。

## 核心与 NAPI 接口

- 新 C ABI `controller_identity_create_v1()` 返回包含 Base64 seed/publicKey 的秘密 JSON，用 `controller_secret_string_free_v1()` 尽力擦除并释放；NAPI `createIdentity()` 转成短期 JS 数据，仅交给 Asset Store。
- 新 C ABI `controller_connection_create_access_v1(request_json, credential_json, timeout_ms)`。公开连接请求沿用 R1；秘密 JSON 为 `{mode,seed,credentialId,credentialSecret}`，pair 后两个字段为空。C++ 与 Rust 对临时秘密副本尽力擦除，不把它拼入公开 profile。
- NAPI `connectTrustedScreen(requestJson, credentialsJson, surfaceId, timeoutMs, callback)` 与原 connectScreen 分离；原接口保持不变。
- 配对回执事件 `access_paired` 附 `credentialId`、`credentialSecret`、`expiresAt`。核心必须校验版本、scope、控制端公钥、长度与仅收到一次。ArkTS 保存成功才显示本机绑定完成，保存失败断开并提示在 Windows 撤销孤立记录。
- `connected` 事件可选 `accessMode`。缺该字段按现有现场批准处理。pair/unattended 必须等真实匹配回执，不能把普通 PeerInfo 当无人值守成功。

## 验证

先写协议/授权行为测试，再实现：有效配对与重新连接、错误控制私钥、旧挑战重放、错目标、修改 scope、错误随机 secret、过期、撤销、损坏文件、密码/密钥/策略变化、2FA 禁止绕过、配对未明确批准。验证已登记文件重载后的成功/撤销；对窗口与令牌全生命周期保留现有输入测试。

运行核心测试、Windows 编译/定向测试、ArkTS 模型与双 ABI/HAP、Flutter 控件测试。系统安全存储和界面现场操作、真实 hbbs/hbbr、公网、真机分别记录，不能用模型或协议 fixtures 冒充验收。
