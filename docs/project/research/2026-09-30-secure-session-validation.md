# 严格身份握手与非媒体登录：实施及测试

日期：2026-09-30。基线：`6a96b474a`。对应 [实施计划](../plans/2026-09-30-secure-session.md)。用户要求继续实现并同步测试，视频和真机验证后移。

## 本轮新增能力

- 独立共享核心可以在明确可信的设备 ID 与 Ed25519 公钥约束下，验证 SignedId、建立 v1 加密通道、读取加密 Hash、发送登录挑战响应并识别对端 PeerInfo 确认。
- 密码通过独立临时参数传入，拒绝进入 profile；Rust 使用 Zeroizing，完成/失败/取消时清理，C++ 临时缓冲也清理。JS 字符串不作物理立即擦除承诺。
- 成功仅返回 `verified=true, authenticated=true, authorized=false`，随即关闭。本轮没有 Active 可操作会话，没有键鼠、文件、终端、隧道或视频能力。
- 鸿蒙页增加可信设备 ID/公钥字段及“验证加密登录”入口；输入本次密码后立即清空，切设备/取消/离页也清理并隔离旧回调。
- 空密码发起现场批准请求并保持等待，收到明确 PeerInfo 才确认登录；2FA 请求明确阻塞，尚未提供继续第二因子的完整交互。

## 上游复用与构建

不修改上游 `src/common.rs` 或 `libs/hbb_common/src/tcp.rs`。build.rs 校验规范化 SHA-256 后提取身份函数、Encrypt 与 nonce helper 到 OUT_DIR，运行时代码原样使用；提取的上游测试补充 sodium 初始化。

上游固定 v0/v1 wire 向量、双向密钥差异、transcript 绑定、版本与短公钥测试随独立库运行。严格登录目前选择 v1 最低策略；v0 是旧加密方式而非明文，两者不能混称。

`sodiumoxide 0.2.7 / libsodium-sys 0.2.7` 与上游依赖体系一致。OHOS 使用绑定包所附 libsodium 1.0.18 源码以及固定 CMake 包装提交构建；来源哈希、许可和重复构建步骤见 [native/README.md](../../../libs/controller-core/native/README.md)。生成库、下载缓存均忽略。

Windows 构建脚本的 MSVC cfg 会让 libsodium-sys 在 OHOS 目标也查找 `libsodium` 名称，因此提供原 archive 的相同字节别名 `liblibsodium.a`。构建期间显式设置目标 SODIUM_LIB_DIR，退出后恢复进程环境；空环境变量也会被视为已设置，必须真正移除废弃 SODIUM_STATIC。

## 验证结果

| 测试组 | 数量 | 结果 |
| --- | ---: | --- |
| 复用上游帧编解码 | 8 | 通过 |
| 复用上游加密固定向量及方向/transcript/版本 | 7 | 通过 |
| profile/C ABI/只读预检 | 8 | 通过，保留原功能回归 |
| 签名身份与封装密钥 | 3 | 通过 |
| 严格会话受控 loopback | 12 | 通过 |
| 合计 | 38 | 全部通过 |

真实 socket 的受控对端用官方 protobuf 与相同上游加密实现构造报文，并由独立固定向量约束加密兼容性。成功用例验证服务端能够解密登录请求、密码摘要正确、危险操作选项关闭、收到加密关闭消息；失败用例检查不会得到认证或操作权限。

重点覆盖：错误参数、公钥/签名错误、设备 ID 错误、旧版本拒绝、明文首包零凭据发送、错误密码、损坏密文、单字节密文绕过拒绝、重复加密挑战重放拒绝、超时、连接后取消、2FA 阻塞、空密码等候明确确认。取消用例要求连接后的等待在 1 秒内结束。

执行命令：

```powershell
cargo fmt --manifest-path libs/controller-core/Cargo.toml --check
cargo test --manifest-path libs/controller-core/Cargo.toml --locked
& ./apps/harmony-controller/scripts/build.ps1
```

OHOS x86_64、ARM64 静态库、NAPI `.so` 及 ArkTS/HAP 构建通过。环境沿用 DevEco SDK 6.1.1.125/API24，compatible API22。本轮不进行视频解码或真机测试，也没有将新登录链路在模拟器执行为成功声明；HAP 是未签名开发产物。

## 审查修正

1. 独立库绕开原应用初始化，现已在创建认证任务时调用 sodiumoxide::init，失败拒绝启动。
2. 切换设备时取消旧任务并更新 epoch，避免 A 设备的登录结果归到 B 卡片。
3. 失败或取消不等待任务对象最终销毁才清理密码；已提取的密码由局部 Zeroizing 清理。
4. 登录请求成功发送后才进入 authenticating/awaiting_approval；PeerInfo 前 authenticated=false，全程 authorized=false。

修正后独立只读复核未发现新增明确 P1/P2。这不是完整产品安全审计，也不表示可以公网发布。

## 必须保留的限制

- 当前成功证据是主机上的受控协议对端，不是实际官方 Windows 被控应用互通。官方现有 direct_server 使用 secure=false，不发该签名握手；严格入口会拒绝它。
- 仍需接入实际 ID/中继通道，或给本产品被控端实现受约束的签名直连入口；完整官方 Session/Remote 未整体移植。
- `PeerInfo` 表示登录确认，不等于键鼠授权。桌面服务端可能默认不发送正向权限位，后续需明确权限快照协议，不能用缺省值打开操作。
- 原协议没有 disable_video。登录可能触发对端屏幕订阅，本版本不解码且确认后立即关闭，不能宣传网络上完全没有视频。
- 当前控制方 my_id 是验证阶段占位标识，不能当作已注册或具密钥证明的控制设备身份；设备归属与持久控制身份仍需后续实现。
- C ABI 的 nonce 溢出有拒绝门禁，但本轮未直接对会话的 u64 耗尽分支做运行测试；不以测试数量声称穷尽所有安全情况。

## 回归范围

改变的现有产品路径仅限本分支新增的 controller-core 配置、版本/能力声明、预检结束提示、构建入口，以及鸿蒙控制端的桥接与资料/状态页面。旧资料通过新增字段的默认空值兼容，密码仍不进入资料保存。

根 RustDesk `src/`、原有 `libs/` 模块、子模块和根 Cargo 未改动。后续本机测试资料、HAP、原始日志与密钥不提交 Git。
