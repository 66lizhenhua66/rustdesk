# 2026-10-07 鸿蒙真机首次连接验证记录

日期:2026-10-07。代码基线:`feat/harmony-controller` 分支 `846e37687`(无源码改动,本轮为构建环境重建与真机现场验证)。记录人辅助:Claude Code 会话。

## 本轮范围

在新整机环境(无历史构建缓存)重建双端构建链路,完成签名 HAP 真机安装与首次 IP 直连只读会话。本轮不改变任何产品源码;仓库内改动仅为本记录及相关文档。

## 构建环境重建(新机器,Windows 11)

- Rust:rustup stable,默认工具链切 **stable-msvc**(Windows 构建脚本依赖 host=msvc);OHOS 双目标(x86_64/aarch64-unknown-linux-ohos)与 x86_64-pc-windows-msvc 均安装。
- 代理与镜像:`127.0.0.1:7890` 本机代理;rustup/cargo 走 rsproxy 镜像;crates.io CDN 与 GitHub 大文件经代理均出现 TLS 闪断,源码包以手动预下载 + 哈希校验方式落缓存。
- MSYS2(含 make/perl)与 mingw-w64-binutils(dlltool)安装;`mingw perl`(5.44,含 lib 树)移植入 vcpkg 工具目录替代 strawberry-perl 下载;libyuv 的 googlesource 以 git `insteadOf` 重写至 GitHub 镜像(lemenkov/libyuv,提交哈希一致);LLVM 以 `C:\llvm` junction 复用 DevEco 自带 clang 15.0.4(vcpkg CLANG 定义无版本强校验)。
- VS Build Tools 2022(VCTools)安装;Windows 开发人员模式开启(Flutter Windows 插件符号链接要求);`core.symlinks=false` 解决 pub git 依赖(uni_links)symlink 检出。
- 关键脚本运行环境:构建/启动脚本须以 **pwsh 7** 运行(`IPEndPoint.Parse` 在 PS 5.1 不存在;内嵌引号参数在 PS 5.1 会被拆散)。
- vcpkg:固定提交 `9e593bb18` 全部 16 项 `x64-windows-static` 依赖本地编译通过(含 ffmpeg/amf/nvcodec/qsv);blobless 克隆的历史端口对象按需补齐。

## 构建产物与安装证据

- 鸿蒙:`build.ps1 -DevEcoRoot 'C:\Program Files\Huawei\DevEco Studio'` 产出双 ABI HAP。工程级 `build-profile.json5` 的 `products` 补 `signingConfig` 引用后 hvigor 正常签名(本地改动,不入 Git),`entry-default-signed.hap`(约 20.8 MB)。
- 真机安装:`hdc app install` 返回 `install bundle successfully`;`bm dump -n com.openremotedesk.controller` 正常;`aa start -a EntryAbility` 返回 `start ability successfully`,应用前台运行。
- Windows:`build-windows-host.ps1 -Configuration Debug -GenerateBridge -LibClangPath C:\llvm\bin` 产出 `flutter/build/windows/x64/runner/Debug/`(rustdesk.exe / librustdesk.dll / flutter_windows.dll 齐全)。cargo 限流 `CARGO_BUILD_JOBS=10` 后在 32GB 内存下稳定完成。

## 首次连接流程(现场操作)

被控端:`start-windows-host.ps1 -Configuration Debug -Video -Input -Listen 192.168.1.16:21120 -Allow 192.168.1.10`,server(隐藏)与 Flutter 工作台(可见)两进程启动,`netstat` 确认 `192.168.1.16:21120 LISTENING`。

控制端(真机,与被控端同网段)在 Desk 工作台导入被控端导出的 `official-current-profile.json`(direct 模式,peerId `442402027`、固定公钥与指纹);导入由 `uitest` UI 自动化完成:粘贴 JSON → 验证并预览(界面显示的公钥/指纹与服务端导出一致)→ 确认保存,设备卡显示「IP 直连 · 现场批准后仅查看」。

点击「连接桌面 →」后,界面依次出现并保持:「可信身份与加密通道已核验」「等待本机批准查看 / 请在 Windows 连接管理器中接受请求」。用户在 Windows Flutter 工作台点击批准后,真机进入只读会话并显示被控主屏。**画面、批准流程与整体会话由用户现场确认「测试看起来没啥问题」;本记录未采集逐帧计数、光标细节与断开清屏证据。**

## 通过项(附证据)

| 项 | 证据 |
| --- | --- |
| 新整机双端构建链路可复现 | HAP 与 Windows Debug 整包均产出并运行 |
| ARM64 真机签名安装、冷启动 | hdc `install bundle successfully` / `aa start` 成功,应用前台可用 |
| 公开配置导入与信任材料核对 | uitest 注入 → 预览公钥/指纹与服务端导出一致 → 保存成功 |
| IP 直连身份验证与加密握手 | 界面「可信身份与加密通道已核验」 |
| 现场批准 → 只读画面 | 用户在 Windows 工作台批准,真机显示主屏(用户确认) |

## 未覆盖项(不宣称通过)

- D01 的覆盖安装、重复退出重进;D02 的新增/编辑/删除/搜索/常用与冷启动持久化复验。
- D03 的拒绝、超时、取消、断开重连、错误身份;D04 的首帧时间、静止画面、窗口变化、光标位置、断开清屏逐项证据。
- 键鼠 D06—D12:本轮虽以 `-Video -Input` 启动,但未执行控制切换、触控板、中文输入与释放清理的任何验收。
- D13 长时运行指标,D14—D17(ID/中继与可信设备无人值守)未动。
- 真实 hbbs/hbbr、公网、硬件解码均未涉及。

## 环境备注(复现要点)

- 详细环境事实(路径、镜像、junction、PATH 陷阱、pwsh 要求)记录于开发机本地,不在仓库展开;复现以各脚本与 [入口说明](../OFFICIAL-SECURE-ENTRY.md) 为准。
- `apps/harmony-controller/artifacts/` 下本轮的 layout 转储与临时脚本为本地调试材料,不入 Git。
