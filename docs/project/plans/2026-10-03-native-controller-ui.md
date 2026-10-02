# 鸿蒙正式界面实现计划

日期：2026-10-03。基线：`feat/harmony-controller` / `181fe5c21`。用户已授权按当前原型写计划后直接实施，保留全部原型、签名及本地工具配置；完成后仅本地提交。

## 目标与边界

以当前 `prototypes/2026-09-30-remote-desk/` v3 为视觉依据，交付可运行的 ArkUI 原生工作台、真实设备管理与现场批准后的只读桌面。原型 README 尚未选定 A/B/C，本轮采用其推荐 A；三者共用的会话设计保持一致。原型是设计参考，其模拟能力不是后端能力。

ArkUI → 既有 NAPI/C++ → Rust 会话核心保持不变；VP8 解码和像素始终在原生 Surface。新增正式页面，旧 `pages/Index.ets` 作为连接诊断保留，不重写已验证的 DEMO。Windows 视频门禁默认关闭，只有受控联调显式启动 `-Video`，不自动批准、不扩大监听、不修改系统设置。

## 原型映射

| 原型 | 原生实现与接线 | 本轮验收 |
| --- | --- | --- |
| A 设备工作台 | `pages/Desk.ets`：标题、浅绿欢迎卡、连接卡、搜索、设备卡、空状态；沿用 `controller_profiles/profiles` 与 Rust validateProfile | 已有资料读取；新增、编辑、删除、导入预览、冷启动保留；不伪造在线状态 |
| 连接卡/设备详情 | IP 与端口分字段，可信 ID/公钥核对；保存不连接，连接只调用 connectScreen | IPv4/IPv6/端口交给既有 Rust 验证；缺信任材料不可连接；不静默更换绑定身份 |
| 等待批准 | 真实 connecting/verified/awaiting_approval 事件映射三阶段；提供取消 | 未批准无画面，取消清资源；拒绝、超时、身份错误可见 |
| 远程会话 | `components/RemoteSession.ets`：稳定 XComponent、浮动栏/胶囊、工具覆层、真实统计 | 批准且收到呈现事件后显示真实桌面，保持比例；返回设备不改活动目标，第二连接先要求断开 |
| 结束页 | 清 Surface 和权限，显示原因、重试/返回 | 断开清屏、重试新 generation 且重新批准；旧回调忽略 |
| 最近连接 | 仅存真实获批连接记录，独立非敏感 preference key | 不存密码、画面、输入；空记录不显示样例 |
| 偏好设置 | 连接服务/远程桌面两张白卡；已实现能力和限制可见；诊断入口 | ID/中继、无人值守、控制、键盘、文件、音频、画质切换禁用/未实现，不模拟成功 |

视觉使用主绿 `#196B52`、文字 `#20352F`、辅助 `#778780`、底色 `#F6F8F5`、边框 `#E4EAE5`、欢迎卡 `#E6EEE1`、深绿导航 `#20382E`，14–18 vp 卡片圆角和至少 44 vp 操作区。<600 vp 手机底栏/单列；600–1099 vp 导航轨/双列；>=1100 vp 完整侧栏/三列。横竖屏以当前窗口宽高决定，弹窗与工具可滚动；视频保持源比例，不拉伸。

## 实施顺序

- [x] 读取项目约束、规格、上轮验证；检查 Git、完整原型文件与实际页面、手机和平板会话、设置及 Windows 页。
- [x] 新增 `model/DeskModels.ts`：Profile/ScreenState 类型、核心事件到 UI 的展示投影、IP/端口组装与拆分；针对未批准/终止清状态/重连及 IPv6 写行为测试（Node 内置测试，无新依赖）。UI 不实现第二套认证状态机。
- [x] 新增 `components/RemoteSession.ets` 与小型图形资源：按原型实现会话视图，Surface 由父页面持有；工具仅呈现真实能力，无输入调用。
- [x] 新增 `pages/Desk.ets`：复用现有设备存储键、校验与导入流程；新增正式首页和表单；稳定持有单个会话、epoch/taskId 过滤，后台/退出主动取消，返回设备不毁 Surface。
- [x] `EntryAbility.ets` 与 `main_pages.json` 仅切换默认页面并保留旧诊断页面；诊断导航前结束正式会话，返回重新加载设备。
- [x] 执行 `node --experimental-strip-types --test apps/harmony-controller/tests/*.test.ts`、`cargo test --manifest-path libs/controller-core/Cargo.toml --locked`、`cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked`；必要时运行现有 Windows 门禁/视频回归。
- [x] 执行 `apps/harmony-controller/scripts/build.ps1`，核对双 ABI/HAP；用 SDK HDC 安装到现有 API22 x86_64 模拟器，保留已有资料/转发。
- [x] 模拟器截图核对手机横竖屏、2in1 多窗口宽度、表单/工具滚动、等待/失败/断开；合成视频会话、工具、旋转、返回保持 Surface 通过。正式 host、安装和请求已准备后才向用户请求 CM 接受。
- [x] 独立代码复核与最小回归面检查；更新 PROGRESS、应用 README 与验证记录；精确暂存本轮文件，本地提交不推送，刷新已有 GitNexus 索引（不存在 CodeGraph，不创建）。

## 官方资料与 API 约束

通过项目 `harmonyos_developer_knowledge` 的 searchDocuments/getDocumentsById 核验：基础 onAreaChange API8；XComponentController Surface 回调 API12；Preferences API9。目标 SDK API24、兼容 API22；不使用 onAreaChange 的 API26 新参数。具体来源与实际构建/设备证据记入验证报告。既有唯一 INTERNET 权限保持。

## Windows 界面后续任务

1. 将本机访问/访问记录/安全设置外壳接入正式 CM，显示真实请求来源、校验结果和查看范围。
2. 本次活动会话与远程访问总开关分开，批准查看/拒绝/断开沿现有执行点。
3. 独立系统键鼠切片：Windows 本机明确授予、执行点撤权、释放按键、断开清理；控制方按钮不能自行授权。
4. 无人值守、可信设备登记/撤销、凭据轮换另行实现和安全验收，首次安装默认关闭。
5. 上传/下载按方向与目录范围独立授权、撤权及任务终止；不从原型复制示例文件或进度。

## 回归面与保护

预期既有代码改动只有鸿蒙启动路由与页面清单；新增页面复用既有 API，不修改 Rust、Windows 默认路径、NAPI、视频 renderer。若构建/实际行为需要额外接线，在验证记录说明必要性。用户 `build-profile.json5`、`.clangd`、`.clang-tidy` 与整个原型目录不暂存；先记录哈希，结束对比。截图和含真实桌面的材料只留本机忽略目录。

- [ ] 本轮真实 Windows 桌面人工复验：等待用户在本机 CM 接受，批准后核对真实画面及断开/重新批准。请求超时不算通过；真机继续后移。
