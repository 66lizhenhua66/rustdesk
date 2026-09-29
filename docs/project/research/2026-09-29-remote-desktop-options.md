# 开源远程桌面选型调研

调研日期：2026-09-29。目标：以 Windows 为被控端，兼顾 macOS；控制端包括电脑、Android、iPhone/iPad，特别关注 HarmonyOS 6 原生应用；用途是协助操作对方正在使用的屏幕。

本报告为官方文档及项目仓库调研，没有进行设备实测。原生客户端、浏览器客户端和 Android APK 兼容层是不同路线，不能互相替代兼容性结论。

## 选型比较

| 方案 | Windows / macOS 被控 | 手机和平板主控 | 鸿蒙 6 路线 | 开源与自建 | 对当前需求的判断 |
| --- | --- | --- | --- | --- | --- |
| RustDesk | 两者均有正式平台支持；控制当前屏幕 | Android、iOS/iPadOS 官方客户端 | 官方平台表未列 HarmonyOS；社区原生移植见下节 | 客户端与 OSS 服务端 AGPL-3.0；Pro 另付费 | 常规远程协助首选候选；鸿蒙需核实社区客户端 |
| MeshCentral | Windows、macOS 均有 Agent | 浏览器远控，无需安装主控客户端 | 浏览器候选，未验证鸿蒙 6 实机 | Apache-2.0；可自建完整管理、桌面、终端、文件服务 | 适合当前屏幕协助、多设备管理；缺少已核实的原生鸿蒙端 |
| Sunshine + Moonlight | Windows 支持；Sunshine 安装文档仍将 macOS 标为实验性 | Android、iOS/iPadOS、Windows、macOS 等客户端 | 官方平台表未列 HarmonyOS；社区移植见下节 | 两者 GPL-3.0；自主搭建串流 | 更适合自己的 PC、低延迟画面；协助流程不如 RustDesk 对口 |
| Apache Guacamole | Windows 通过 RDP/VNC；Mac 可评估 VNC 接入 | HTML5 浏览器，官方有触屏操作说明 | 浏览器候选，未验证鸿蒙 6 实机 | Apache-2.0；自建网关和后端协议服务 | 普通 RDP 不应当作共享当前屏幕协助；若选它应评估 VNC 路线 |
| noVNC + VNC 服务端 | 由 VNC 服务端决定；Windows 可搭配 UltraVNC | 现代手机/平板浏览器 | 浏览器候选，未验证鸿蒙 6 实机 | noVNC 主要 MPL-2.0；UltraVNC GPL | 适合定制嵌入浏览器，需自己补网关、认证及设备管理 |

来源：[RustDesk 平台](https://rustdesk.com/docs/en/client/)、[RustDesk 代码](https://github.com/rustdesk/rustdesk)、[MeshCentral](https://meshcentral.com/)、[MeshCentral 仓库](https://github.com/Ylianst/MeshCentral)、[Moonlight](https://moonlight-stream.org/)、[Sunshine 仓库](https://github.com/LizardByte/Sunshine)、[Sunshine 安装文档](https://docs.lizardbyte.dev/projects/sunshine/latest/md_docs_2getting__started.html)、[Guacamole](https://guacamole.apache.org/)、[noVNC](https://github.com/novnc/noVNC)、[UltraVNC](https://uvnc.com/)。

## RustDesk：最接近常见远程协助的软件形态

- 官方客户端覆盖 Windows、macOS、Linux、Android、iOS；iOS 是主控端，不能被远控。Android APK 由 GitHub 下载，iPhone/iPad 由 App Store 下载。[平台和安装](https://rustdesk.com/docs/en/client/)
- 客户端及免费 OSS 服务端采用 AGPL-3.0。OSS 服务端包含 hbbs 信令/发现和 hbbr 中继，优先尝试直接连接，无法直连时转中继；可以自行部署。[客户端仓库](https://github.com/rustdesk/rustdesk)、[服务端仓库](https://github.com/rustdesk/rustdesk-server)、[自建架构](https://rustdesk.com/docs/en/self-host/)
- Windows 远程协助涉及 UAC 时，免安装运行可能出现画面冻结/不能控制管理窗口；官方建议安装被控端服务或使用提权流程。这是验收重点，不应只验证普通桌面点击。[官方 FAQ：Windows UAC](https://github.com/rustdesk/rustdesk/wiki/FAQ#windows-uac)
- macOS 需屏幕录制、辅助功能权限，部分输入场景需要输入监控权限；不能认为安装后无需现场授权即可被控。[macOS 文档](https://rustdesk.com/docs/en/client/mac/)
- **Web Console 是管理后台，Web Client 才是浏览器发起远控。** 官方当前价格页注明，Pro 集成 Web Client 要求套餐满足 `(用户数 × 10) + 设备数 ≥ 400`；不可将“RustDesk 开源”理解成“最新官方浏览器版本也可无限制免费开源自建”。本次没有充分证据确认当前官方 Web Client 的独立源码许可，不能给它套用整个客户端仓库的 AGPL 结论。[价格及 FAQ](https://rustdesk.com/pricing.html)
- 官方平台支持页未列原生 HarmonyOS；这只说明官方支持列表，不能据此排除第三方原生移植。[平台页](https://rustdesk.com/docs/en/client/)

## MeshCentral：浏览器操作当前桌面的候选

- 官方定义为可自建的 Web 设备管理系统；在被控电脑安装 Agent，浏览器提供远程桌面、终端和文件管理。[仓库说明](https://github.com/Ylianst/MeshCentral)
- 官方站点明确服务器和管理 Agent 支持 Windows、Linux、macOS、FreeBSD，项目采用 Apache-2.0。[官网](https://meshcentral.com/)、[文档](https://docs.meshcentral.com/)
- 对手机/平板可采用浏览器路线，但“Web 技术可运行”不足以证明鸿蒙 6 已适配：触摸鼠标、输入法、软键盘、剪贴板及后台恢复仍需实机测试。没有核实到官方原生鸿蒙客户端。

## Sunshine + Moonlight：低延迟串流候选

- Sunshine 是被控端串流服务器，Moonlight 是主控端客户端。Moonlight 官方支持 Android、iPhone、iPad、Windows、macOS 等，并明确支持完整 Windows 桌面串流，不只游戏。[Moonlight 官网](https://moonlight-stream.org/)
- Sunshine 支持 AMD、Intel、NVIDIA 硬件编码及软件编码。其 Web UI 用来配置和配对，**不是浏览器远程桌面客户端**。[Sunshine 仓库](https://github.com/LizardByte/Sunshine)
- Moonlight 与 Sunshine 均为 GPL-3.0；Moonlight 明确免费、无功能订阅。跨公网可以配置端口转发、IPv6 或虚拟网络，部署路径与输入 ID 的远程协助工具不同。[Moonlight 官网](https://moonlight-stream.org/)
- Sunshine 当前安装文档仍把 macOS 标为实验性，并说明权限及部分按键限制。文档与仓库 README 对手柄能力存在更新节奏差异，应以选定发行版验证，不能宣称与 Windows 完全对等。[安装文档](https://docs.lizardbyte.dev/projects/sunshine/latest/md_docs_2getting__started.html)
- 官方 Moonlight 平台清单未列 HarmonyOS 原生版；社区移植需要单独核实维护状态、安装包以及许可。[平台列表](https://moonlight-stream.org/)

## 浏览器补充路线

- Guacamole 是 HTML5 远程访问网关，支持 RDP、VNC、SSH。官方用户指南覆盖触屏、绝对/相对鼠标、屏幕键盘、剪贴板，以及取决于后端协议的文件传输。[官网](https://guacamole.apache.org/)、[操作指南](https://guacamole.apache.org/doc/gug/using-guacamole.html)
- Windows 家庭版不支持作为 Microsoft RDP 被控端；Pro、Enterprise、Education 和 Windows Server 支持。且普通 Windows RDP 登录与“协助对方一起看当前屏幕”语义不同，不能因为 RDP 可连接就视为满足需求。[微软说明](https://learn.microsoft.com/en-us/windows-server/remote/remote-desktop-services/remotepc/remote-desktop-allow-access)
- noVNC 主要采用 MPL-2.0，是浏览器 VNC 客户端而非完整远控平台；通常搭配 VNC 服务端和 WebSocket 转换代理 websockify。官方列明现代移动浏览器支持，但不等于已验证 HarmonyOS 6 浏览器。[noVNC 仓库](https://github.com/novnc/noVNC)

## 鸿蒙原生移植核实

以下是社区项目，不是 RustDesk/Moonlight 官方原生鸿蒙支持；本次均未使用鸿蒙 6 真机验证。Android APK 兼容性也不能据此推断为 HarmonyOS 6 原生支持。

| 项目 | 一手资料所述能力 | 安装与成熟度 | 适配需求判断 |
| --- | --- | --- | --- |
| [RemoteDeskHarmonyOS](https://github.com/Mydstiny/RemoteDeskHarmonyOS) | ArkTS/ArkUI 原生手机、平板、PC；包含 RustDesk/RDP/VNC/Moonlight；RustDesk 提供 ID、中继、直连、画面及键鼠；AGPL-3.0-or-later | README 有测试发布验证及 unsigned HAP 信息；本次 GitHub Releases API 查询为空，未确认现成生产安装包 | 功能方向最符合远程协助，但应当作为实验候选 |
| [rustdesk_harmonyos](https://github.com/liyan-lucky/rustdesk_harmonyos) | ArkTS + NAPI + Rust 核心的第三方原生 RustDesk 端；AGPL-3.0-only | README 标注 0.35.23（2026-09-17）、HAP-only 侧载及“目前只能测试用”；本次公开 Releases 查询为空，不能保证直接下载 HAP；文件传输等仍需回归 | 与 ID 式协助需求贴近，暂不作为稳定成品承诺 |
| [Moonlight V+ / moonlight-harmony](https://github.com/AlkaidLab/moonlight-harmony) | 原生 HarmonyOS NEXT 5+ 手机、平板；触控鼠标和完整键盘；GPL-3.0 | README 提供 [华为应用市场链接](https://appgallery.huawei.com/app/detail?id=com.alkaidlab.sdream)，本次未亲验商店上架及安装；标准串流可接官方 Sunshine，部分增强依赖 Foundation Sunshine | 原生安装路径更明确，适合事先配对的 Windows；协助陌生/临时设备不如 RustDesk 流程直接 |
| [moonlight-harmonyos](https://github.com/likuai2010/moonlight-harmonyos) | 另一社区移植路线 | README 进展资料较旧，本次未核实为成熟发行版 | 不作为优先推荐 |

读取来源为各项目仓库 README。空 Releases 查询仅表示本次公开 GitHub Releases 接口没有结果，不证明仓库中绝无构建产物或没有其他发布渠道。核查接口：[RemoteDeskHarmonyOS releases](https://api.github.com/repos/Mydstiny/RemoteDeskHarmonyOS/releases?per_page=1)、[rustdesk_harmonyos releases](https://api.github.com/repos/liyan-lucky/rustdesk_harmonyos/releases?per_page=1)。

### 针对当前需求的排序

1. **需求匹配优先：评估两个 RustDesk 兼容原生鸿蒙项目。** Windows 端用 RustDesk；优势是 ID、穿透/中继及远程协助形态，但鸿蒙端现阶段需要接受测试/自行构建的成本。
2. **先追求可安装的原生应用：尝试 Moonlight V+ + Sunshine。** 更适合自己或固定对象的电脑提前部署、配对和配置网络。Sunshine 官方提供 Desktop 空命令示例，能够串流当前桌面，并不局限游戏。[桌面配置示例](https://docs.lizardbyte.dev/projects/sunshine/latest/md_docs_2app__examples.html)
3. **原生方案不稳定时：MeshCentral 浏览器兜底。** 免费开源自建、当前桌面协助和管理功能齐全；必须用实际鸿蒙浏览器验证输入体验。

当前没有足够证据认定某一款同时满足“官方成熟支持鸿蒙 6 原生、完整开源、开箱即用 Windows 当前屏幕远程协助”。社区原生路线已经存在，不能简单回答“鸿蒙只能用浏览器”。

## 建议的验收项

针对实际鸿蒙 6 手机/平板验证：原生应用能否安装与启动；连接 Windows 当前会话；触摸点按、右键、滚动与拖拽；中文输入与软键盘；蓝牙鼠标键盘；横竖屏及分辨率；UAC 窗口操作；剪贴板/文件传输；公网连接与断线重连。以这些结果判断可用程度，而不是只依据项目名称或支持平台宣称。
