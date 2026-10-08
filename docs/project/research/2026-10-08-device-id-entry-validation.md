# 设备 ID 入口验证记录

日期：2026-10-08。基线：`4d4e1424d`。用户确认无现成 hbbs/hbbr，先完成代码和本地验证；真机验收后移。

## 交付行为

- 首页原静态标签改为设备 ID、IP 直连和安全中继的真实入口；按保存方式和完整 ID 筛选，未保存的 ID 引导添加或导入。
- 同 ID 对应多份服务资料时必须选择具体条目，候选项显示服务地址。连接始终使用原保存 profile，不临时改写 mode 或设备公钥。
- 添加时预填当前方式和 ID，保存/导入后同步首页，普通连接仍默认控制、可选仅查看，正在连接时返回原会话。
- Windows ID 公开配置采用独立且包含服务地址摘要的本地标识，可以与 IP 配置及其他服务下相同 ID 的配置并存。设备公钥和指纹不变，重复导入不覆盖已有信任。

操作说明：[通过设备 ID 连接](../DEVICE-ID-ACCESS.md)。首次仍需导入或手动核对设备公开身份，单独输入一个未保存的 ID 不会建立信任。

## 自动化与构建

| 验证 | 本轮结果 | 范围 |
| --- | --- | --- |
| controller-core 全量测试 | 130 项通过 | 包含 11 项协调/中继协议测试、4 项 Windows rendezvous 策略/导出测试，以及原输入/画布/会话回归 |
| Harmony 模型与接线测试 | 25 项通过 | 新增 3 项方式隔离、未知 ID、同 ID 多服务的行为测试 |
| Harmony assembleHap | BUILD SUCCESSFUL | 新 ArkTS 页面编译；输出未签名 HAP，无新增系统 API 或权限 |
| Windows Debug 整包 | 构建成功 | Rust 库、Flutter runner 和完整输出目录 |
| 独立 diff 审查 | 未发现阻塞项 | 首页候选选择、会话/pin 保持、导出身份隔离与回归面 |

复现命令（仓库根目录）：

```powershell
cargo test --manifest-path libs/controller-core/Cargo.toml --locked --offline
node --experimental-strip-types --test apps/harmony-controller/tests/desk-models.test.ts apps/harmony-controller/tests/input-models.test.ts apps/harmony-controller/tests/access-models.test.ts apps/harmony-controller/tests/session-input.test.ts apps/harmony-controller/tests/device-id-models.test.ts
& 'C:\Program Files\PowerShell\7\pwsh.exe' -NoProfile -File scripts/build-windows-host.ps1 -Configuration Debug -Offline
```

HAP 使用 DevEco 6.1.1.125 的 Node/Hvigor 在应用目录执行 `--mode module -p module=entry@default -p product=default assembleHap --no-daemon`。产物：`apps/harmony-controller/entry/build/default/outputs/default/entry-default-unsigned.hap`。本轮 Rust 生产变化仅在 Windows 导出模块，移动端 Rust/NAPI 沿用前轮双 ABI 产物，不把本轮 HAP 编译写成重新完成双 ABI 构建。

Windows 首次链接被此前开发 EXE 占用；核对两个进程均来自本仓库 Debug 目录、全部 TCP 无已建立连接后停止，重建通过，再恢复原 `127.0.0.1:21120` 的 `-Video -Input` 开发入口，没有自动批准连接。Flutter/Dart 用户缓存需要沙箱外读写。构建保留现有警告，未做无关清理。

Harmony 文档核对：使用官方知识 MCP 检索并读取 [TextInput](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-basic-components-textinput) 与 [Select](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/ts-basic-components-select)，复用已有 `onChange`、`onSelect`、`selected` 和 `value`；没有引入版本更高的接口。

## 官方服务包与实服验证限制

通过 [官方 Release API](https://api.github.com/repos/rustdesk/rustdesk-server/releases/tags/1.1.16) 获取 1.1.16 Windows unsigned 资产，asset ID 为 `483984088`。归档大小 11,909,626 字节，SHA-256 与 GitHub 公布值一致：

```text
b865a3a62fc8755b45480c508f1c4871c3338590408dda8c58c7e9c373b7adb0
```

缓存仅位于忽略目录 `apps/harmony-controller/artifacts/id-relay-validation/`。`hbbs.exe` 和 `hbbr.exe` 的 `--version` 均为 1.1.16。已执行 `--help`：公开选项中无明确 bind/listen 地址参数。源码查询受到 GitHub API 速率限制及 raw/codeload 网络不可达影响，不能据此断言不存在其他配置方式，但未能确认如何仅监听回环。Docker daemon 未运行，WSL/Windows Sandbox 也没有可用的隔离运行条件。

因此本轮没有启动真实 hbbs/hbbr，不把发行文件校验或本地协议 fixtures 记为真实服务兼容性通过。未修改防火墙、未部署外部服务、未使用公共 RustDesk 服务。

已通过的协议测试覆盖协调 TCP、强制中继、TCP 失败后中继、错服务签名/错目标身份拒绝、取消与超时。它们使用受控本地对端，不能证明发行服务器注册/心跳、实际 NAT、移动网络或画面输入链路已通过。

后续需要在可隔离的自建 hbbs/hbbr 环境完成 Windows 注册、ID 直连、强制中继、失败回退与现场批准后的画面/输入；再执行公网和真机验收。

## 回归面与保留文件

现有运行路径仅改动 `Desk.ets` 的首页选择、添加预填和保存/连接后的首页同步，以及 `secure_rendezvous.rs` / `secure_rendezvous_policy.rs` 的 ID 公开配置导出。新增模型只负责选择保存资料，未改变网络协议、设备身份校验、现场批准、输入或 IP 传输路径。

新增测试验证 API 结果和导出的公开资料，不增加测试基础设施。所有变更经最小化审查；用户已有签名配置、构建脚本修改、`.clangd` 和 `.clang-tidy` 保留，未纳入本轮提交。
