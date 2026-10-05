# ID/中继与可信设备只读访问验证

日期：2026-10-05。实现基线 `88e2387e0`；本记录对应其后的本轮变更。用户已确认先完成 ID/中继，再实现可信设备无人值守，并同步界面与文档。

## 已实现与验证范围

- 鸿蒙新增 ID 自动连接、强制中继、完整服务配置、实际路径事件和具体失败提示。核心复用上游 protobuf/BytesCodec/Encrypt，签名校验 hbbs、校验其签名的目标公钥，再进行既有目标 SignedId/KX1。只允许 TCP 建连失败转固定中继，身份错误直接停止。Windows 安全入口新增显式自建服务注册、协调 TCP 与固定 hbbr 接入；两种接入使用同一个连接计数器。
- 新增可信设备登记、每连接新挑战的 Ed25519 持有证明、随机访问凭据、30 天只读授权、到期/策略变化失效和撤销。普通批准不会登记；Windows 必须显式启用 `-Unattended`，并选择「批准并登记只读访问」。已配置 2FA 时拒绝这条尚无 2FA 继续能力的路径。
- 初版配对/无人值守使用 `secure_video`，无法协商或开启键鼠。只读连接固定最多 15 分钟，心跳不续期；授权失效和撤销每秒检查。Windows 存储签名记录与 secret 摘要，跨进程互斥并原子替换。撤销全部可清空损坏或旧密钥记录，单项读取/授权遇到损坏则拒绝。
- 鸿蒙种子和目标绑定的凭据由 Asset Store 保存；普通 profile/Preferences 不包含秘密。设备卡显示本机登记状态和到期时间。配对回执成功持久化后才显示登记成功；异步保存期间禁止相冲突的删除/重新登记，删除固定操作开始时的目标 ID。Windows 提供登记列表和撤销；成功状态以实际存储结果为准。

这些是源码、构建、模型与本地协议对端的验证，不是现实网络和真机验收。

## 自动化与构建证据

| 检查 | 结果 | 本机日志（忽略目录） |
| --- | --- | --- |
| `cargo test --manifest-path libs/controller-core/Cargo.toml --locked` | 107 项通过 | `artifacts/trusted-access-core-final.log` |
| 旧 DEMO `cargo test --manifest-path apps/windows-demo-host/Cargo.toml --offline` | 9 项通过 | `artifacts/trusted-access-demo-final.log` |
| 鸿蒙 `desk-models`、`input-models`、`access-models` Node tests | 20 项通过 | 本轮命令输出 |
| `scripts/build-windows-host.ps1 -Configuration Debug -Test -Offline` | Windows 15 项、Flutter 6 项通过；定向 analyze 无问题 | `artifacts/trusted-access-windows-tests.log` |
| Windows `-GenerateBridge -Offline` 与 Debug 整包 | FRB、Rust DLL、Flutter EXE/资源通过 | `artifacts/trusted-access-windows-build.log`、`trusted-access-windows-build-final.log` |
| OHOS x86_64、aarch64 release 静态库 | 两种 ABI 通过 | `artifacts/trusted-access-hap-build-final.log` |
| Hvigor `assembleHap --no-daemon` | NAPI/ArkTS/HAP 通过 | `artifacts/unattended-harmony-report.md`；实际工具执行输出 |

自动化合计 **157 项通过**。上述 `artifacts/` 均相对于 `apps/harmony-controller/`。最终 HAP 为 `entry/build/default/outputs/default/entry-default-unsigned.hap`；Windows 正式整包位于 `flutter/build/windows/x64/runner/Debug/`。HAP 未签名，不能直接据此认为真机可安装。

本轮产物 SHA-256：EXE `238C570B1F79B9CE8E2941EFE270E912CF8F471FE525F85DF2AF0CD57BFAA897`；DLL `7141BFB718B72BEDB382446864FA66D4AA24270F9C2B94F8A2ACB62373D665A6`；HAP `83268861A9B0344E7F9C42B1E9E49BBDB7B12CAC7D7D33D84CFD91CA6E7E2444`。收尾恢复原回环 `127.0.0.1:21120` 的 Video/Input 测试服务与工作台，无人值守开关仍为关闭；没有配置或连接公网服务。

回环协议测试覆盖：协调直连、强制中继、建连失败回退、完整目标 KX1、错误服务签名/目标绑定/实际目标身份、非零时间戳地址编码、取消、超时、超大帧；配对及无人值守回执、跨目标/跨 challenge/跨 scope 重放、错误 secret、过期、策略变化、记录重载/撤销/损坏与恢复、普通批准和 2FA 不能绕过、只读不能提权。

本轮曾发现并修正：profile 新字段被原严格 schema 拒绝、地址解码混入时间戳、并发配额耗尽关闭服务、CM 来源状态漏传、错误码映射不一致、秘密临时缓冲缺少尽力擦除、Asset 异步写入与删除竞争、删除目标在 await 后变化。独立审查最终无未解决项，报告保存在 `artifacts/id-relay-review.md` 和 `artifacts/unattended-review.md`。

构建中遇到 Hvigor 用户缓存沙盒权限、ArkTS 禁止直接重抛任意值/对象展开、旧 EXE 占用，分别通过构建权限、符合 ArkTS 的显式写法和核对无活动连接后停止本仓库测试进程解决。旧 DEMO 的锁文件补入 core 新增 socket2 的必要依赖闭包；其产品代码未改，离线回归通过。未更新 Flutter/SDK 或 hbb_common 子模块。

## 仍待现场验证的事项

- 真实 hbbs/hbbr 的版本兼容、注册/长连接、跨网 NAT、IPv6、自定义端口与来源过滤；本机没有已配置的自建实例，Docker daemon 不可用，未部署公网服务。
- API22 模拟器与 HarmonyOS 6.1 真机的 Asset Store 添加/查询/删除、首次解锁/锁屏、重启与卸载；官方文档及 SDK 编译只确认接口可用性。
- 真实 Flutter CM 登记、HAP 持久化、退出重进后的无人值守查看、实际视频会话中的撤销/过期，以及保存失败时的孤立授权处理。未自动批准新请求，也未替用户登记任何控制设备。
- 信令失联时当前入口停止并需要手动重启；仅 TCP 协调，无 UDP/WebRTC。固定允许来源由可信 hbbs 提供的原始控制端 IP 校验，动态来源需要本机更新白名单。
- 这是独立、固定公钥的严格入口，不是完整上游 Client/Session 移植。无人值守只有只读能力；完整 2FA 继续流程、无人值守键鼠、文件传输、声音、剪贴板、硬解和公网发布门槛仍未完成。
- JS 不可变字符串、protobuf/系统加密内部副本不能保证即时物理擦除；已对本次直接管理的 Rust/C++ 秘密缓冲尽力擦除，不声称所有进程内副本均已清除。

## 回归面与最小化核对

以下既有文件的修改均用于本轮功能；新增实现放在对应的 `rendezvous`、`access` 和平台存储模块。

| 既有文件 | 必须改变的路径/原因 |
| --- | --- |
| `libs/base/protos/message.proto`、`libs/base/src/lib.rs` | 增加版本化配对字段与共享签名材料入口，不改旧字段编号 |
| `libs/controller-core/src/lib.rs`、`src/session.rs`、`include/session.h` | 连接模式、profile 中继字段、新 C ABI、严格访问证明/回执、实际路径事件；旧 direct 和普通批准分支保留 |
| `libs/controller-core/Cargo.toml`、`Cargo.lock`、`apps/windows-demo-host/Cargo.lock` | 同端口 TCP 协调需要 socket2，更新两个独立工作空间的必要锁定闭包 |
| `src/server.rs`、`src/server/secure_host.rs` | Windows/feature 门控模块和显式服务接线；原受限 IP 监听仍可单独使用，ID 与监听共享 ServerPtr |
| `src/server/connection/secure_host.rs` | 严格入口的新鲜 challenge、独立登记/无人值守准入、只读生命周期和撤销；普通输入/批准路径不重写 |
| `src/flutter_ffi.rs`、`src/ui_cm_interface.rs` | 专用本机 CM 管理入口、显式登记命令和状态回执；普通 CM 不获得这些管理接口 |
| `flutter/lib/models/server_model.dart` | 把两个新增可信访问状态传给工作台，包括尚待批准的状态更新 |
| `flutter/lib/desktop/pages/secure_host_page.dart`、`widgets/secure_host_workspace.dart` | 独立登记按钮、凭据状态/到期/撤销、区分现场批准与可信只读连接 |
| `apps/harmony-controller/entry/src/main/cpp/controller_napi.cpp`、`types/libcontroller/Index.d.ts` | 增加秘密隔离的身份/可信连接接口，沿用 Job/Surface 清理，秘密事件副本尽力擦除 |
| `apps/harmony-controller/entry/src/main/ets/pages/Desk.ets`、`model/DeskModels.ts`、`components/RemoteSession.ets` | ID 表单与实际路径、显式可信访问、平台安全存储和只读状态呈现 |
| `apps/harmony-controller/entry/src/main/ets/pages/Index.ets` | 旧诊断资料编辑/复制时保留新增中继字段，防止丢配置；不改旧连接执行流程 |
| `scripts/start-windows-host.ps1`、`start-official-secure-host.ps1` | 显式服务/中继/无人值守参数、环境恢复及公开 profile 导出；默认不启用新能力 |
| 两个已存在的 UI 测试文件、项目进度/入口/README | 固定可见行为与更新能力边界，无未实现能力伪装成完成 |

原有用户签名配置、`.clangd`/`.clang-tidy` 和原型目录保留；未将它们当成本轮改动纳入验证。`scripts/build-official-secure-host.ps1` 原有工作区换行状态没有本轮内容变更。测试产物/日志、生成桥接和本地授权文件不作为产品源文件提交。
