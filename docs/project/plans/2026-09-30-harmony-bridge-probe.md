# T0/T1 鸿蒙原生桥接验证计划

日期：2026-09-30。用户已授权开始验证；沿用 feat/harmony-controller。这里只验证独立 ArkUI/NAPI/Rust 工程，不接入 RustDesk 网络或新增公网入口。

目标：可重复编译并在当前模拟器运行版本查询、后台进度、取消、页面销毁与重新进入。用户真机可用但当前连接的是 x86_64 / API22 模拟器；ARM64 只作交叉编译，真机结果单列。

## 环境与范围

- DevEco: G:/Huawei/DevEco Studio；SDK24 6.1.1.125，工程 target 6.1.1(24)、compatible 6.0.2(22)。
- Rust 1.96.1；新建无三方依赖 Rust crate，独立 workspace，避免下载和编译整个 RustDesk。
- 只新增 apps/harmony-probe/ 与文档，不改现有 src/libs/Cargo 或子模块。
- .so/.a、构建缓存、签名材料、本机路径配置及未脱敏设备日志忽略，不提交。

## 任务与契约

- [x] T0 发现工具链和模拟器，确认 API/ABI。
- [x] T1a Rust C ABI：apps/harmony-probe/native/rust，Cargo.toml/src/lib.rs/include/probe.h。导出 probe_version、probe_create(steps, interval_ms)、probe_run(task, callback, user)、probe_cancel(task)、probe_destroy(task)。一次 run 顺序回调 progress(step,total) 与 completed/cancelled；cancel 唤醒等待；destroy 仅在 worker 停止后调用。参数约束 steps=1..100、interval=10..1000ms；无网络、无设备数据。先以3个行为测试证明正常完成、取消后及时退出、非法参数拒绝，记录红/绿。
- [x] T1b C++ NAPI：entry/src/main/cpp。导出 version/start(steps,interval,callback)/cancel(taskId)/dispose。每个环境只运行一个任务；创建 TSFN，后台线程只持 native 数据；开始新任务前停止旧任务。cancel/dispose 先通知 Rust 再 join（Rust 等待可立即取消，不依赖 UI 回调），再 abort/release TSFN 和 destroy；call 失败自行释放 payload，CallJs 必须处理 env/jsCallback 为 null。回调含 taskId，UI 丢弃旧事件。env cleanup 同样停止线程，禁止线程继续触碰已关闭 TSFN。
- [x] T1c ArkUI 工程：AppScope/entry/hvigor 配置和 Start/Cancel/Dispose/再次进入页面。Native callback 只更新低频状态，不做生产远控。提供自动验证入口，验证正常完成、取消、快速重启、重复销毁和旧任务事件过滤，界面显示结果并有脱敏hilog标记。
- [x] T1d 可重复构建脚本：显式SDK/Node/Java/Hvigor，Rust target按x86_64/arm64构建staticlib，CMake按ABI链接；参数允许覆盖路径。检查 native ABI/导出，再 assembleHap。
- [x] T1e 模拟器：检查是否存在同包名后安装独立 probe 包；启动并读自动验证结果，必要时UI验证。未具备合法签名时准备可签名产物与明确阻塞，不读取/复制已有私钥密码。
- [x] T1f 独立复核、格式/测试/产物验证、记录结果与边界，提交到当前分支，不推送。

## 验证命令与证据

- cargo test --manifest-path apps/harmony-probe/native/rust/Cargo.toml（使用独立target目录）；需要Windows linker时优先现有VS。
- apps/harmony-probe/scripts/build.ps1：双ABI Rust库及 HAP，失败保留日志在忽略目录。
- llvm-readelf/llvm-nm 验证 .so 架构与Rust/NAPI链接。
- hdc 安装/aa start；hilog只筛当前 probe marker；测试不含密码、账户、远控数据。
- 真机没有运行就不标真机通过；桥接成功不标官方核心、认证、视频或打洞可用。
