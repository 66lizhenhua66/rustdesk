# 控制端非媒体基础：实施记录

日期：2026-09-30。基线：`23f85e74f`，分支：`feat/harmony-controller`。用户要求做好计划直接开始，视频解码与真机验证后续再做。对应 [实施计划](../plans/2026-09-30-controller-foundation.md)。

## 已交付

1. 独立 `libs/controller-core`：跨平台 profile 校验、C ABI、有限只读 TCP 预检，直接复用 `libs/hbb_common/src/bytes_codec.rs` 和 `libs/base/protos/message.proto`。
2. `apps/harmony-controller`：ArkUI 设备资料新增/编辑/删除，本地 Preferences 保存，IP/设备 ID/中继配置，中文错误提示及网络预检页面。
3. NAPI 网络操作非阻塞取消和生命周期：单活动任务、最多 4 个未退出 worker；TSFN owner/worker 引用和 shared ownership；关闭后丢弃旧事件。
4. 双 ABI 构建脚本与独立依赖锁文件，SDK24 target/API22 compatible。

本轮没有移植完整官方 `Session/Remote`。网络预检不是远控：不发出应用载荷，不提交密码，始终 `verified=false/authorized=false`，没有键鼠、终端、隧道、视频或权限执行能力。

## 官方核心移植诊断

- `rustc --print cfg --target x86_64-unknown-linux-ohos` 实际输出 `target_os="linux"`、`target_env="ohos"`。因此必须按 target_env 区分，不能只假定 OHOS 是一个新的 target_os。
- 根 Cargo 即使 `--no-default-features`，仍无条件引入 `scrap/wayland`、`hbb_common/webrtc` 和 Linux GTK/PulseAudio/D-Bus/X11 等桌面依赖。
- `src/lib.rs` 仍选择 Linux 桌面 UI、被控服务与 IPC；`Client::start` 只建立连接流，认证和消息循环在 `Session<T>` 与 `Remote::io_loop`。
- `Remote::new` 会启动音频，收到视频帧会创建解码线程；推迟视频需要真实的模块/能力隔离，不能通过空 UI 回调假装已经移植。
- `scrap/build.rs` 对 libyuv/libvpx/aom 等有构建要求；尚无可直接复用的无媒体 controller-only 入口。
- 本机 `cargo check --offline --locked -p rustdesk --no-default-features --lib --target x86_64-unknown-linux-ohos` 和 `-p base` 在缺失 pinned `tungstenite` Git checkout 处退出 101，尚未进入 Rust 编译。上述平台依赖问题来自源码检查，不是伪称编译器已经逐一报出。

诊断日志仅在本机 `apps/harmony-probe/artifacts/cargo-*.log` 等文件保留。没有为消除诊断失败而改动原有 Cargo、子模块或密码学。

## 验证结果

| 验证 | 结果 | 边界 |
| --- | --- | --- |
| Rust 主机测试 | 15/15：8 个复用帧编解码测试、7 个本轮 ABI/行为测试 | 本机 Windows，不是真机 |
| 地址/资料 | 规范化 IP、拒绝未知秘密字段、公钥/指纹格式检查 | 资料校验不等于身份已验证 |
| loopback 预检 | 测试服务器读取不到发送载荷；有效首帧也不授权 | 只验证预检接口，不是实际 RustDesk 登录 |
| 空/unknown/超大帧 | 不宣称协议身份；超出 64 KiB 拒绝 | 不接收媒体流 |
| 取消 | 开始前取消及 TCP 已建立等待帧时取消均通过 | C++ 不 join UI；连接系统调用仍受至多 2 秒超时约束 |
| Rust fmt | 通过 | 原始子模块源码未修改 |
| OHOS x86_64/ARM64 | release staticlib 与 NAPI `.so` 链接成功 | ARM64 未上真机 |
| ArkTS/HAP | BUILD SUCCESSFUL | 开发 unsigned 包；SDK24、兼容 API22 |
| 模拟器配置冒烟 | 新增、保存、回环预检、进程停止后重启读取、删除测试记录通过 | 无视频，无真机；编辑流程已实现但未单独 UI 验收 |
| 独立只读复核 | 无需修正的本轮 P1/P2 | 不等于全产品安全审计 |

模拟器为此前用户启动的 x86_64/API22/OpenHarmony-6.0.2.130。测试配置 `ControllerSmoke / 127.0.0.1:1` 仅用于验证，已通过应用 UI 删除。未连接用户远端电脑或第三方公网目标。

目前已知构建提示：复用上游 BytesCodec 的 set_raw 在本库未使用；HAP 无 signingConfig。前者不影响当前路径，后者意味着仅开发包，不可视为正式签名分发产物。

## 下一阶段

1. 新增 OHOS controller-only 构建边界，排除 Linux 桌面与被控路径；保持原平台默认路径。
2. 将官方 `Session/Remote` 的网络认证循环与音视频初始化分离，禁用媒体时明确不启动其线程。
3. 接入同一个共享核心/桥接接口，验证 ID/中继以及产品的安全 IP 直连，不重写密码学或用 TCP 可达替代认证。
4. 实施被控端权限与公网门槛；视频解码和真机验收按用户指示后移。

本轮是可运行的控制端基础，不是 M1 完整远控交付。原有 RustDesk 运行路径未改变，新增库独立 workspace；根 `src/`、既有 `libs/` 和子模块无差异。
