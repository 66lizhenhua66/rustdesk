# 官方 Windows 安全入口验证记录

日期：2026-09-30；基线 `a4b7f6a26`；分支 `feat/harmony-controller`。用户选择先接正式 Windows 安全入口，真机验证跳过但未标为通过。

## 交付与实际验证

已增加默认关闭的 `ord-secure-host` feature：专用 TCP 监听、来源 IP 白名单、签名及严格 v1 握手、官方 Connection/CM 的独立非媒体审批路径。旧自动密码/最近会话/可信标签/切换方向授权不进入新分支；所有权限 false，不创建媒体服务或订阅。停止服务开关会停止监听及会话；重启后需重新批准。

| 验证 | 本轮结果 | 证据边界 |
| --- | --- | --- |
| 共享核心全套测试 | 50 项通过 | 原 44 项 + 5 项正式入口策略/密码学测试 + 1 项正式 profile 只读连接回归，后者直接编译正式源码、复用实际上游 protobuf/Encrypt |
| 原 Windows DEMO 回归 | 9 项通过 | 真实 TCP/受限演示权限，未把它当作官方 CM 互通 |
| 官方审批状态单测 | 1 项通过，344 项被 filter 排除 | 证明 pending/重复批准/2FA 状态约束；不是实际 CM GUI 点击测试 |
| 官方 Windows feature-off 编译检查 | 通过 | `cargo check --locked -p rustdesk --lib --no-default-features` |
| 官方 Windows feature-on 编译检查 | 通过 | 同命令加 `--features ord-secure-host`，实际编译所有新增入口模块 |
| 官方 Windows 开发版链接 | 通过 | `scripts/build-official-secure-host.ps1 -DebugBuild` 生成 `target/debug/rustdesk.exe` |
| 鸿蒙 → Release 正式入口 pending | 通过 | 新 HAP 导入 `ord_secure_host` profile 后，签名/加密完成，页面显示等待 Windows CM 批准；未收到批准前保持未授权 |
| 格式与独立源码复核 | 通过 | 新模块 rustfmt、diff 检查；两次独立复核涵盖策略、入口、CM 分流和停止服务处理 |

合计运行 **60 项测试**。普通构建有 40 条上游警告；安全配置因为提前分流增加不可达/未使用提示（开发版共 46 条）。没有为清理警告而改写原有业务路径。没有执行全部官方 345 项单测，也没有完成 CM 窗口点击批准、正式画面/输入或公网验收。

开发版 EXE SHA256：`de24d1da9419e4b9cfaa272aa78d731d4a2368e5bbae0a49ae1c118df5c6a457`。旁边补齐上游 README 指向的 Sciter 运行时，固定到 `c-smile/sciter-sdk` 提交 `f33df075d9eb2f8d252cb88f1b2c8096e56197ed` 的 `bin.win/x64/sciter.dll`，SHA256 为 `4d97528e157c55ef1fabe9e37a9697116ab66660d7da6163f90a3a7abf80dd56`。Release Connection Manager 窗口可启动；Debug 构建会在旧 rust-sciter wrapper 的组合 enum 转换处 panic。临时缓存诊断补丁已恢复，未进入仓库。

## 构建环境补齐

首次官方构建先因缺 pinned Git checkout 停止；联网取得 Cargo.lock 依赖后，暴露缺少 vcpkg 与 libclang。没有关闭这些依赖或使用空实现绕过编译。

在 `apps/harmony-controller/artifacts/official-build/vcpkg` 准备了仓库固定的 vcpkg 提交 `9e593bb18ea69cc5095e012465dcd675a822ed0d`，按原 `vcpkg.json` / overlay 安装 16 项 `x64-windows-static` 依赖。浅克隆缺历史端口对象的问题通过补齐原提交历史解决，未改变锁定版本。较短的构建树位于外层工作空间 `.native/buildtrees`，没有加入 Git。

使用 Visual Studio 2022 Build Tools 及 DevEco LLVM 15.0.4 的 libclang。DevEco Clang 默认目标为 GNU Windows，直接调用曾找不到标准 C 头；构建脚本现在显式选择 MSVC 目标、Clang resource-dir，并加载 VS 的 INCLUDE/LIB 等环境，结束后恢复进程变量。最终 feature-off/on 检查和开发版链接均成功。

日志保留在本机忽略的 `apps/harmony-controller/artifacts/`：`official-core-regression.log`、`official-demo-regression.log`、`official-host-root-test.log`、`official-host-baseline-final.log`、`official-host-feature-final.log`、`official-host-debug-build.log`。下载/原始编译日志与文件哈希清单位于 `official-build/`。

## 复核结果与回归范围

新 listener 的逐连接失败日志已降为 trace，避免 peer-controlled warn 写盘；补充了对既有 stop-service 的遵守。监听和空闲会话每秒检查停止，正在进行的握手/发送仍受自身超时限制，不承诺一秒硬关闭。当前范围独立复核无剩余可操作问题。

已有产品源码的改动仅为 43 行新增：`Cargo.toml` 增加 feature；`src/rendezvous_mediator.rs` 增加 Windows 安全构建的启动分流；`src/server.rs` 增加模块和强制严格握手分流；`src/server/connection.rs` 增加产品状态及独立循环调用。feature 关闭时原代码继续执行，没有替换原实现。完整逻辑放在三个新增 `secure_host*` 模块中。

未改上游密码学、proto 或子模块，未改已有鸿蒙应用运行代码。用户 DevEco 生成的签名/IDE 配置和其他任务的界面原型未纳入本轮提交。

## 后续验证

下一步需要在可枚举的 Windows UI 自动化环境中点击正式 CM 批准并观察控制端收到 PeerInfo；当前协议 pending 已证明连接和权限前置逻辑，CM 点击因本机 Computer Use 未枚举窗口而未完成。之后再接视频及系统输入的执行点授权与撤权。当前“批准”只输出非媒体能力信息，不能打开键鼠；主 SPEC 的短期协助凭据且现场批准、完整可信设备/2FA、公网策略、服务安装/旧版本共存和安全审计仍未验收。极短报文兼容限制见 [入口说明](../OFFICIAL-SECURE-ENTRY.md)。
