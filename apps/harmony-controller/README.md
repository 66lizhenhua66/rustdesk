# 鸿蒙控制端：非媒体基础

当前可用：设备配置的新增、编辑、删除与本地保存；IP/设备 ID/中继三种配置；共享 Rust 校验；用户主动发起的 IP 单点网络预检。

**当前不能进行远程桌面会话。** 设备 ID/中继只保存配置；IP 预检不发送密码或任何应用协议请求，不验证对端身份、不授予权限。完整 RustDesk Session、加密认证、输入、视频、音频与文件通道尚未接入。

## 结构

- `entry/src/main/ets`：ArkUI 界面与 Preferences 非敏感设备资料。加载失败显示错误并禁止覆盖，未知字段或密码字段不能通过核心校验。
- `entry/src/main/cpp`：NAPI 与 Rust C ABI。网络操作在后台，取消/销毁只标记停止，不在 UI 线程 join；工作线程结束后释放其 TSFN 引用和任务。
- [libs/controller-core](../../libs/controller-core/README.md)：共享配置与预检逻辑，直接复用上游帧编解码源码和 protobuf 定义，未复制一份协议文件。
- `scripts/build.ps1`：构建独立核心的 x86_64/ARM64 库和 HAP。

原有 `apps/harmony-probe` 保留作为第一阶段桥接验证；没有改动根 `src/`、既有 `libs/` 模块或根 Cargo workspace。

## 构建与测试

从 RustDesk 仓库根目录运行：

```powershell
cargo test --manifest-path libs/controller-core/Cargo.toml --locked
cargo fmt --manifest-path libs/controller-core/Cargo.toml --check
& ./apps/harmony-controller/scripts/build.ps1 -DevEcoRoot 'G:\Huawei\DevEco Studio'
```

环境与前一轮 probe 相同：Rust 1.96.1，DevEco 内置 SDK24/API24，HAP 兼容 API22。依赖版本由独立 Cargo.lock 固定。需要已安装 x86_64-unknown-linux-ohos、aarch64-unknown-linux-ohos 标准库。初次构建需要下载公开 Cargo 依赖。

开发包：`entry/build/default/outputs/default/entry-default-unsigned.hap`。本机 API22 模拟器已安装并完成基本配置冒烟检查；真机与视频按用户要求后移。unsigned 包不能当作可对外分发的签名版本。

## 使用边界

IP 配置接受标准 literal IPv4/IPv6，未填端口默认 21118；带端口 IPv6 使用 `[地址]:端口`。设备 ID 当前限定 6—20 位数字。服务器地址可保存域名，默认端口 21116，但不会据此发起设备 ID 会话。

公钥和指纹都是公开信任材料；保存它们不等于本版本已完成身份校验。此版本不提供密码字段，也不保存凭据。

“网络预检”只连接你指定的 IP，最多等待 2 秒，最多解析 64 KiB 首帧。即使能连接、收到符合上游格式的消息，结果仍保持未验证、未授权，并以 `AUTH_BACKEND_NOT_READY` 结束。取消立即返回；正在进行的系统 TCP connect 最迟到其有限超时结束，不能解读为底层线程已瞬间退出。

每个环境最多允许 4 个尚未退出的预检 worker，旧回调在退出/取消后被丢弃。它不是端口扫描器，不自动探测公网或保存原始远端报文。

本地构建和冒烟证据在忽略的 `artifacts/`，不提交截图、设备布局、原始日志、产物或签名文件。验证摘要见 [实施记录](../../docs/project/research/2026-09-30-controller-foundation-implementation.md)。
