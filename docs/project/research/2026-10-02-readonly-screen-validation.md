# Windows → HarmonyOS 只读画面验证

日期：2026-10-02。分支 `feat/harmony-controller`，起始提交 `2bdb98094`。本记录对应 [实施计划](../plans/2026-10-02-readonly-screen.md)，不替代真机、公网和产品验收。

## 实现范围

Windows 安全入口新增本机 `ORD_SECURE_VIDEO=1` 门禁，客户端须明确请求 `secure_video`/VP8，并经过原 CM 的逐连接批准。批准后才枚举主屏、创建采集与编码器。GDI 缩放至不超过 1280×720，约 8 fps，相同像素不编码；使用现有 scrap VP8 编码和标准 VideoFrame，而非逐帧 PNG/JPEG。

共享核心使用同步借用压缩帧的 C ABI；鸿蒙以 ArkUI XComponent + C++ libvpx 1.15.2 + NativeWindow/NativeBuffer 显示。像素不进入 ArkTS/JSON。libvpx 双 ABI 由固定 SHA512 源归档构建，来源/许可证见 native README。键鼠、文件、音频等能力仍关闭。

协议在批准前维持 64 KiB 包上限；确认视频能力后视频包不超过 8 MiB，每个 VP8 帧不超过 2 MiB、每包最多 4 帧。控制消息仍按原始密文长度检查 64 KiB，未知 protobuf 字段不能绕过。拒绝非预期编解码、显示编号、首个非关键帧和权限提升；native 进一步检查关键帧头和实际解码尺寸。

## 自动化与构建

本轮最终主机测试：**77 项通过、0 失败**。

| 命令/范围 | 结果 |
| --- | --- |
| `cargo test --manifest-path libs/controller-core/Cargo.toml --locked` | 63 项；含旧会话回归、视频前置审批/模式、帧边界、取消及超限控制包 |
| `cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked` | 9 项旧 DEMO 回归 |
| `scripts/build-official-secure-host.ps1 -TestVideo` | 4 项；尺寸、显式开关、实际 VP8 编解码、满队列停止 |
| `scripts/build-official-secure-host.ps1 -TestSecureGate` | 1 项真实 Connection 审批门禁测试 |
| `scripts/build-official-secure-host.ps1 -DebugBuild` | 正式 Windows Debug EXE 成功 |
| `apps/harmony-controller/scripts/build.ps1` | Rust/libvpx/C++ 双 ABI 与 HAP 成功 |

构建首次遇到用户目录 `.hvigor` 写权限限制，按构建所需权限重试后通过；没有修改系统安全设置或全局 Cargo 源码。旧测试进程占用 EXE，停止已确认路径/PID 的旧测试实例后链接成功。

原始日志位于忽略目录 `apps/harmony-controller/artifacts/video-*-final-test.log`、`video-official-build.log`、`video-hap-final-build.log`。独立代码复核覆盖 host/core、NAPI 锁顺序与取消、surface 生命周期、解码边界；原超限控制包问题已补回归并复核关闭。

## 模拟器实测

设备为已有 OpenHarmony 6.0.2/API22 x86_64 模拟器；**不是 HarmonyOS 6.1 ARM64 真机**。HAP 覆盖安装成功，保留原 DEMO 与正式设备资料。

`libs/controller-core/examples/screen_fixture.rs` 是仅绑定 loopback 的测试发送器：使用临时身份、真实 v1 加密协议传输 64×48 合成 VP8 关键帧，不抓桌面、不实现正式 CM 批准。测试端初版 accepted socket 继承 nonblocking，导致 Windows 10035；显式恢复阻塞读取后连接成功。

模拟器观察到收到 110 帧、native 显示提交计数超过 100，断开后 XComponent 消失且进程仍运行。两种计数独立限频，不保证同一时刻相等。

本轮复现并修复了用户观察到的黑屏：第 0 帧缓冲为 64×48/stride256；ArkUI 收到视频尺寸后调整布局，第 1—7 帧缓冲变成 1058×794/stride4232，旧逻辑只写左上角 64×48。单独调整 scalingMode 时机和显式 renderFit 均未解决，未保留无效 ArkUI 修改。最终在每次 RequestBuffer 前重设视频 geometry，并 Abort 仍不匹配的 buffer，避免将部分绘制的缓冲提交。移除临时 HiLog 诊断与依赖后双 ABI HAP 再次构建/安装成功；`video-fixed.jpeg` 显示灰色合成帧完整铺满区域。灰色是测试输入，不是真实 Windows 桌面。

已通过 UI 删除本轮创建的 7 条临时测试 profile，移除临时端口映射，保留原 Windows DEMO 和正式设备。正式测试入口固定 21120，通过 `start-official-secure-host.ps1 -Video` 启动，仍须本机用户批准。

## 正式桌面联调

本机用户实际点击 CM 接受后，模拟器显示真实 Windows 主桌面 **1152×720**；HDC UI 读到收到 510 帧、呈现统计 504 帧、2475 KiB，随后截图呈现统计为 512 帧，画面可见当前电脑窗口和模拟器递归画面。该计数只是当次观察，不是吞吐/延迟基准。原始截图 `video-official.jpeg` 仅留在本机忽略目录，不进 Git。

点击断开后，UI dump 中 XComponent 数量为 0。再次点击查看桌面，状态重新为等待本机批准，收到/显示均 0 帧、0 KiB；上次批准没有自动复用。取消此次 pending 连接后 XComponent 仍为 0，应用保持运行。为保留真实现场门禁，没有自动代点第二次接受；收尾时 Windows 被控端仍运行，控制端已断开，可由用户重新连接。

## 待验收与已知限制

- 正式 CM 批准后真实桌面、客户端断开和重连重新等待批准已实测；捕获线程资源释放由 Worker Drop/满队列取消测试与代码复核覆盖，尚未做长时间资源曲线。CM 拒绝按钮、锁屏/UAC、安全桌面切换和 stop-service 的实际操作尚未验收。
- 首帧提交时如 NativeWindow 暂无缓冲，仍解码但跳过显示；静止源可能要等下一次变化才显示。后续补单帧缓存/重绘前应独立设计。
- 主屏单一路径，无光标叠加、系统输入、多屏、窗口串流、音频、硬件解码、画质切换、ID/中继和公网验收。
- HAP 未签名包可安装到当前模拟器；ARM64 编译不代表 6.1 真机可安装或运行通过。

## 回归影响范围

- `src/server.rs` 仅增加 Windows/feature 条件下的新模块声明；默认上游路径保持原行为。
- `src/server/connection/secure_host.rs`、`secure_host_policy.rs` 仅扩展既有专用安全入口：批准后 VP8 分支；不调用旧输入/文件/媒体订阅逻辑。
- `libs/controller-core/src/session.rs` 增加视频请求、校验和 C ABI；旧 DEMO/非媒体分支保留，原始控制包限长在共享解码处加固。
- Harmony NAPI、类型声明与 ArkUI 新增独立查看按钮/Surface 生命周期；既有诊断和 DEMO 保留，取消时同时释放新视频资源。
- CMake、HAP 构建与 native 构建脚本增加所需 VP8/NativeWindow 链接；Windows构建脚本增加测试选项，启动脚本增加默认关闭的 `-Video`。
- 用户本地签名配置、`.clang*`、原型目录不进入本轮提交；上游密码学与 protobuf 源、子模块未修改。

## 官方 API 依据

本轮通过项目 Huawei MCP 阅读 [XComponent 指导](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/napi-xcomponent-guidelines)、[NativeWindow](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/capi-external-window-h) 和 [NativeBuffer](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/capi-native-buffer-h)：CreateFromSurfaceId/Destroy 引用、Request/Flush/Abort、fence 关闭、CPU_READ Map/Unmap、API12 起可用和线程安全要求。诊断缩放时另核验 SetScalingModeV2 与 XComponent 默认 renderFit；文档依据不替代模拟器截图验证。
