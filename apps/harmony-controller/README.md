# 鸿蒙控制端：非媒体基础

当前可用：设备配置的新增、编辑、删除与本地保存；IP/设备 ID/中继三种配置；共享 Rust 校验；IP 单点网络预检；预存可信设备身份后的单次加密登录验证。

**当前不能操作远程桌面。** 设备 ID/中继仍只保存配置；IP 预检不发送应用载荷。单独的加密登录入口会验证身份、建立加密并提交本次凭据，登录确认后立即关闭；不授予键鼠等权限。完整官方 Session、ID/中继通道、输入、视频、音频与文件通道尚未接入。

## 结构

- `entry/src/main/ets`：ArkUI 界面与 Preferences 非敏感设备资料。加载失败显示错误并禁止覆盖，未知字段或密码字段不能通过核心校验。
- `entry/src/main/cpp`：NAPI 与 Rust C ABI。网络操作在后台，取消/销毁只标记停止，不在 UI 线程 join；工作线程结束后释放其 TSFN 引用和任务。
- [libs/controller-core](../../libs/controller-core/README.md)：共享配置、预检和严格登录验证，复用上游帧/消息定义及按哈希校验提取的身份与加密实现。
- `scripts/build.ps1`：构建独立核心的 x86_64/ARM64 库和 HAP。

原有 `apps/harmony-probe` 保留作为第一阶段桥接验证；没有改动根 `src/`、既有 `libs/` 模块或根 Cargo workspace。

## 构建与测试

从 RustDesk 仓库根目录运行：

```powershell
cargo test --manifest-path libs/controller-core/Cargo.toml --locked
cargo fmt --manifest-path libs/controller-core/Cargo.toml --check
& ./apps/harmony-controller/scripts/build.ps1 -DevEcoRoot 'G:\Huawei\DevEco Studio'
```

环境与前一轮 probe 相同：Rust 1.96.1，DevEco 内置 SDK24/API24，HAP 兼容 API22。依赖版本由独立 Cargo.lock 固定。需要已安装 x86_64-unknown-linux-ohos、aarch64-unknown-linux-ohos 标准库。初次构建下载公开 Cargo 依赖及校验哈希的 libsodium/CMake 来源；下载和原生构建缓存不进 Git。

开发包：`entry/build/default/outputs/default/entry-default-unsigned.hap`。本机 API22 模拟器已安装并完成基本配置冒烟检查；真机与视频按用户要求后移。unsigned 包不能当作可对外分发的签名版本。

## 使用边界

IP 配置接受标准 literal IPv4/IPv6，未填端口默认 21118；带端口 IPv6 使用 `[地址]:端口`。设备 ID 当前限定 6—20 位数字。服务器地址可保存域名，默认端口 21116，但不会据此发起设备 ID 会话。

公钥和指纹都是公开信任材料；保存本身不代表验证成功。直连资料可填写预先从可信来源取得的被控设备 ID、Ed25519 公钥和可选 SHA-256 公钥指纹；具备这些资料后可以显式选择“验证加密登录”。本次密码单独输入，可留空等待现场批准；启动/取消/切设备/离开页面会清空输入，不写入 Preferences。JS 字符串不能保证立即物理擦除。

“网络预检”只连接指定 IP，最多等待 2 秒，最多解析 64 KiB 首帧；即使收到格式正确的消息，也以 `AUTHENTICATION_REQUIRED` 提醒该检查不验证身份。“验证加密登录”要求对端首包为能由预存公钥验证的 SignedId，设备 ID/指纹一致且支持密钥交换 v1，再接受加密 Hash 和提交登录。旧明文直连入口、错公钥、篡改密文和不满足必要因子的会话均拒绝。

**普通官方 RustDesk 的直接 IP 入口当前没有该签名握手，本入口会拒绝它。** 当前成功路径由本机受控协议对端测试验证，不代表已完成真实 Windows 被控端互通。上游协议没有 disable_video，登录可能触发对端默认屏幕订阅；本版本不解码，并在确认登录后立即关闭，不能宣称对端完全不发送视频。

页面显示“登录已确认”仍不表示可以操作远端。当前全部事件的操作权限为未授权；2FA 请求会明确阻塞，不继续获取第二因子。UI 默认总时限 10 秒；API 接受至多 30 秒。取消立即返回，已连接 socket 被关闭；进行中的系统 connect 最迟在 2 秒上限结束。

每个环境最多允许 4 个尚未退出的网络 worker，旧回调在退出/取消/切设备后被丢弃。它不是端口扫描器，不自动探测公网或保存原始远端报文。

本地构建和冒烟证据在忽略的 `artifacts/`，不提交截图、设备布局、原始日志、产物或签名文件。基础记录见 [非媒体实施记录](../../docs/project/research/2026-09-30-controller-foundation-implementation.md)，本次登录与测试见 [严格登录验证记录](../../docs/project/research/2026-09-30-secure-session-validation.md)。
