# 官方 Windows 安全入口：非媒体工程阶段

本阶段把安全准入代码放入官方被控端的 `Connection` 和连接管理器（CM）路径。它与 `apps/windows-demo-host` 是两个不同的程序。默认构建保持上游行为；产品专用构建必须使用 `ord-secure-host` feature。当前切片只允许现场批准后的非媒体会话，不支持屏幕、系统输入、文件、音频、终端、剪贴板、提权或无人值守。

## 构建与证据边界

先使用仓库 `vcpkg.json`、overlay 与固定 baseline 安装 `x64-windows-static` 依赖（与 `.github/workflows/flutter-build.yml` 的 Windows 环境一致），准备 MSVC/Windows SDK 和 Windows libclang。然后：

```powershell
& ./scripts/build-official-secure-host.ps1 -VcpkgRoot 'C:\vcpkg' -CheckOnly
& ./scripts/build-official-secure-host.ps1 -VcpkgRoot 'C:\vcpkg'
```

本工作空间已将锁定依赖安装到忽略的 `apps/harmony-controller/artifacts/official-build/vcpkg`，脚本可自动发现这个缓存，因此本机可直接运行 `-CheckOnly`、`-TestSecureGate` 或 `-DebugBuild`。脚本自动加载 MSVC 开发环境并指定 bindgen 的 MSVC 目标/Clang 资源目录，结束时恢复进程环境。其他机器仍需准备自己的 vcpkg 依赖；不提交缓存或修改全局环境。

路径为示例；本机 DevEco 的 libclang 可以复用，但它不提供官方 Windows RustDesk 所需的 vcpkg 媒体库。该脚本不会下载或冒充已经安装依赖。Rust 可执行程序默认仍是上游 Sciter 开发壳，运行时依赖和最终 Flutter 界面不在本切片交付范围。

当前实际验证结果以 [PROGRESS](PROGRESS.md) 和本轮验证报告为准。独立策略测试通过不等于完整官方 EXE 已成功编译或真实 CM 已完成运行验证；不要将上一轮 DEMO EXE 重命名为正式端来替代。

本轮已完成 feature-off/on 的完整 Windows 编译检查和开发版链接，实际程序为 `target/debug/rustdesk.exe`，同目录有 Sciter 运行时。`target/release` 仍需按上面的发布构建命令生成。真实 CM 与 GUI 运行未验收，完整证据见 [验证记录](research/2026-09-30-official-secure-entry-validation.md)。

## 显式开启

2026-10-02 更新：安全构建的 Debug CM 已修复，可以使用 PowerShell 7 从仓库根运行 `./scripts/start-official-secure-host.ps1`，它分别启动 `--server` 和 `--cm`，不会自动批准。默认仅监听本机 21120；公开 profile 路径会打印出来。不要通过无参数 Debug 主窗口启动来验收 CM，其他旧 Sciter 窗口不属于本次兼容修复范围。

在具备正确构建及运行时依赖的机器上，通过本地进程环境开启监听。以下是预期运行步骤，未替代本机完整集成验收：

```powershell
$env:ORD_SECURE_LISTEN = '127.0.0.1:21120'
$env:ORD_SECURE_PROFILE_OUT = "$PWD\secure-host-public.json"
& ./target/release/rustdesk.exe
```

不设 `ORD_SECURE_LISTEN` 时安全构建不监听；其被控启动流程不再启动公共 rendezvous、旧 direct 或 LAN 服务。地址必须为具体 IP 和非零端口；非 loopback 还必须设置逗号分隔的 `ORD_SECURE_ALLOW` 来源 IP（最多 16 个），不接受通配来源。不会自动开防火墙。身份使用本机 Config 已有密钥，公钥导出只含可保存的公开字段；拒绝缺失/不匹配身份，不通过降级保证连接成功。

仍遵守正式程序的 `stop-service` 开关：已停用则不绑定；运行时停用会关闭监听并结束安全会话。监听/空闲会话每秒检查，正在进行的握手和写入还受各自超时约束，因此不是硬性的“一秒断开”承诺。本切片停止后重新开启需重启该进程；暂不实现设置热重载。

使用鸿蒙“导入公开配置”后，点击“连接正式入口”，等待官方 CM 的现场批准。该按钮显式发送 expectedPeer=secure_host；“连接演示端”只接受 DEMO，不能自动切换。正式连接始终不开放输入，任何权限提升通知都会断开。旧“验证加密登录”仍为成功后立即关闭的单次诊断。

## 访问边界

- 握手要求 TCP、有效签名身份和密钥交换 v1。空公钥更新、v0、未知版本和非预期消息直接拒绝。
- 仅接受一次普通远程登录请求，忽略常规 `option`、客户端版本/平台等元数据，不调用旧选项处理；拒绝 OS 凭据、文件/摄像头/终端/隧道登录及可信设备标签。
- 本切片只允许空密码 + 本地批准。已有密码自动批准、最近会话复用、切换控制方向都不可达；配置了 2FA 则拒绝本轮连接，不把点击批准当作绕过必要因子的许可。
- 批准只发送 `ord_secure_host=1, media=false, input_scope=none` 和全 false 权限快照，不进行屏幕订阅、桌面枚举或输入注入。本地权限开关也不能开放本轮尚未实现的能力。
- 保持请求大小上限、既有未认证连接预算及有限会话寿命。下一阶段开放视频/键鼠时必须分别接入执行点授权，并补动态撤权/队列清理验收。

现有上游 Stream 对极短密文存在历史透传兼容逻辑。本轮在安全会话中保守拒绝解码后不超过 16 字节的消息，防止短明文被当作加密指令；这也会关闭使用极短合法心跳/关闭包的连接。当前鸿蒙单次登录请求和正常关闭说明不受该长度限制影响；未来持久正式会话接入时，需要专门完善这个传输边界并做兼容测试，不能移除检查后直接开放操作。

本阶段是安全入口工程接入，不是已完成公网产品。主 SPEC 的“短期协助凭据且现场批准”、完整可信控制身份与审计等门槛继续保留。
