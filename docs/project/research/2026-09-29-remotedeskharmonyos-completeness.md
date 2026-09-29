# RemoteDeskHarmonyOS 完成度调研

调研日期：2026-09-29。对象：[Mydstiny/RemoteDeskHarmonyOS](https://github.com/Mydstiny/RemoteDeskHarmonyOS)，公开 main，README 版本 1.1.5.1。重点：鸿蒙 6 手机/平板通过 RustDesk 协助 Windows 当前桌面。

## 结论

判断为：功能覆盖较广、已有协议与原生桥接代码及构建测试记录，但仍处于集成测试和设备验收阶段。适合作为二次开发候选或开发者试用，不宜直接当成成熟的 ToDesk 替代品。不能根据 README、提交数量或主机单元测试数量给出可靠的完成百分比。

本次读取官方仓库 README、开发指南、用户指南、RDP 状态、PR、发布页、CI 配置以及 RustDesk 部分源码；没有编译、安装、真机运行或性能测量。PR 中测试数字为维护者报告，非本次复跑。

## 核心发现及证据

### 1. 有实现与构建记录，仍缺完整真机验收

- README 声明原生 ArkTS/ArkUI，覆盖 Phone/Pad/PC，支持 RDP、RustDesk、SSH/SFTP、VNC、Moonlight。
- RustDesk FFI 源码包含会话、音视频回调、输入、加密通道及连接策略；connector 有协议握手与输入处理代码。不能把 Cargo.toml 中遗留的 Mock 注释当成整个项目仍是空壳的证据。
- 2026-09-05 合并的 [PR #54](https://github.com/Mydstiny/RemoteDeskHarmonyOS/pull/54) 报告 OhosTestCompileArkTS、signed assembleHap、Light 合规通过，记录 native host 997/997 与 H.265 定向测试 6/6。
- 同一 PR 明确写明 Phone/Pad/PC 行为及受控 IPv6/NAT 拓扑矩阵仍待验收，不能宣称设备成功。H.265 硬解也需核对请求、实际协商编码及活动硬件后端。
- [PR #47](https://github.com/Mydstiny/RemoteDeskHarmonyOS/pull/47) 的旧画面方向修复记录中，PC 模拟器已有诊断证据，但当时最终画面和 resize 验收仍待 Windows 对端批准。此历史记录不能用于断言当前仍存在同一个缺陷。

### 2. 当前 RustDesk 自动打洞能力没有开放

[rustdesk_ffi/src/lib.rs](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/rustdesk_ffi/src/lib.rs) 的 `release_transport_capabilities_v1()` 仅公开：

```text
connection_strategy_mask = FORCE_RELAY | DIRECT_IP
peer_candidate_transport_mask = TCP
nat_traversal_flags = 0
```

`validate_release_connection_strategy()` 会拒绝未开放的 AUTO；连接入口还会拒绝非零 NAT flags，错误说明 UDP/KCP 和 IPv6 candidate advertisement 仍受发布门禁约束。源码注释说明需完成固定服务端及设备矩阵验收后才开放。

[KCP 来源文档](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/docs/compliance/RUSTDESK_KCP_PROVENANCE.md) 同样说明：引入并固定 KCP 依赖不等于启用产品能力。

影响：不能将官方 RustDesk 的完整自动打洞能力直接套到此鸿蒙客户端。按设备 ID 跨网协助，当前应按强制中继路线评估延迟和服务器带宽；直接 IP 需要网络本身可达。这个限制也不等于项目完全不支持 IPv6 地址连接。

### 3. 操作体验正在集中修复，尚无优于 ToDesk 的实测证据

[用户指南](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/docs/app-store/USER_GUIDE.md) 列出触控板/直接触控、远程键盘、自定义快捷键、分协议缩放和滚轮方向、退出保护等，方向符合用户需求。

1.1.4 至 1.1.5.1 的变更包括 RustDesk 手机横竖屏、纹理方向、触控坐标、残留输入释放、二级工具栏、远端到本机剪贴板、H.265 预鉴权偏好等。说明实际交互已有较多工作，但这些也是最近持续修正的区域，不能仅凭功能名称认为体验已稳定。

PC 多显示器同时展示仍属实验功能；不能将 HarmonyOS PC 的能力推断为手机和平板同等具备。

### 4. 没有公开 Release；自行构建门槛较高

本次 [Releases 页面](https://github.com/Mydstiny/RemoteDeskHarmonyOS/releases) 明确显示 `There aren’t any releases here`。README 也写明仍处于测试与发布验证阶段。没有核实公开商店分发渠道；不能推断作者没有任何私下测试包。

[开发指南](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/docs/DEVELOPMENT.md) 要求 DevEco、HarmonyOS SDK、Rust/Cargo、C/C++ 工具链、原生依赖构建、本地配置与签名。它不是只改几行 ArkTS 就能轻松产出的纯前端项目。

### 5. 用户说的“鸿蒙 6”还不足以确认安装兼容

[build-profile.example.json5](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/build-profile.example.json5) 当前 `targetSdkVersion` 与 `compatibleSdkVersion` 均为 `6.1.0(23)`；开发指南又以 API 26 为开发基线，强调应用兼容目标和 native ABI 分别核对。

示例配置不是最终安装包清单，但至少不能承诺所有鸿蒙 6.0 设备可直接安装。试用前须核对设备具体系统/API，以及最终 HAP 的 compatibleSdkVersion 和原生库兼容性。

### 6. CI 绿灯不等于完整远控验收

当前公开 [open-source-compliance.yml](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/.github/workflows/open-source-compliance.yml) 在 push/PR 运行 Light 合规脚本，标签采用 Release 模式，主要核对许可、来源、SBOM、敏感信息等。这个工作流本身不运行完整鸿蒙真机远控测试。

[RDP_STATUS.md](https://github.com/Mydstiny/RemoteDeskHarmonyOS/blob/main/docs/RDP_STATUS.md) 也明确仍需真实 HarmonyOS PC + Windows 10/11/Server 的窗口、缩放、PIP、恢复与输入映射验收。这是其他协议也不能仅按支持列表评价成熟度的旁证。

Issues 页面本次无开放条目，且显示创建受限，不能据此认为没有缺陷或用户反馈。

## 对当前项目的建议

作为二次开发候选：值得进行一次范围明确的验证，优先聚焦 RustDesk 协议及 Phone/Pad 交互，不先接手全部五协议和云同步。

作为直接替代 ToDesk 的成品：现有公开证据不足，不建议未经试用就依赖它提供日常远程协助。

最低验证顺序：

1. 核实设备系统/API，并从确定提交构建、签名、安装。
2. 用自建 hbbs/hbbr 连 Windows 官方 RustDesk，验证设备 ID、密码/现场批准和当前屏幕控制。
3. 测触控点按、右键、拖拽、滚轮、缩放后坐标、中文输入、组合键和横竖屏。
4. 测 UAC 管理窗口、锁屏/重连、前后台恢复与至少一次持续会话；这是本次尚未证实的验收项，不代表已发现失败。
5. 测文本剪贴板与文件传输，记录实际编码、延迟、带宽；分别验证中继及可达 IP 直连。当前不把 AUTO 打洞当作已有能力。

## 访问限制

GitHub API 和部分目录页面此次无法由抓取工具读取，改用可访问的公开文件原文、PR、Actions 和 Releases 页面核对；没有完成全仓库审计。背景调研代理因本机 C 盘空间不足未能启动，本报告由主代理完成；报告保存在仍有空间的 G 盘。
