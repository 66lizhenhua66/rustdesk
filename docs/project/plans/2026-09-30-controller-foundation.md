# 控制端非媒体基础实施计划

日期：2026-09-30。基线：23f85e74f。用户授权直接计划和实施，视频解码、真机验证后移。本轮完成可构建非媒体控制端基础，不把 TCP 可达或协议报文当作远程授权。

## 证据与范围

OHOS 的 target_os=linux/target_env=ohos。上游 root 无条件依赖 scrap/wayland、hbb_common/webrtc，并纳入 Linux 桌面/被控路径。Client::start 仅建立流；Session + Remote::io_loop 才驱动认证且自动启动音视频。离线官方 cargo check 在缺失 tungstenite Git checkout 时停止，未进入编译；不能据此宣称已实际编译验证所有平台阻塞。

本轮复用子模块的原始 BytesCodec 和根 libs/base/protos/message.proto，由独立可移植 crate 编译，不复制维护一份协议。完整 Session 后端仍列后续适配，不重写认证/密码学以绕过依赖。

## 交付

- [x] C1 libs/controller-core：独立 Rust workspace；serde/serde_json、bytes、tokio-util codec、protobuf 3.7 + pure codegen。直接引用上游 BytesCodec 源文件和 message.proto。profile {id,name,mode,target,server,serverKey,peerFingerprint}，mode direct/id/relay；不接受或保存密码。标准IP/端口解析、域名服务地址校验、64位hex指纹格式和服务公钥格式校验；配置缺信任信息可保存草稿，但诊断不宣称已认证。
- [x] C2 C ABI：controller_version，controller_validate_profile(JSON)->owned JSON（ok/profile 或 error/code），controller_free_string，controller_probe_create(endpoint,timeout_ms)->task，controller_probe_run(task,callback_json,user)，controller_probe_cancel，controller_probe_destroy。probe仅对指定 literal IP:port 做有限 TCP 探测与首条上游帧解码，不发送应用载荷/凭据；最大帧64KiB、总时限2秒；每轮返回 connecting/transport_open/protocol_message/blocked/failed/cancelled，始终verified=false,authorized=false。明确AUTH_BACKEND_NOT_READY，不导出输入/文件/提权能力。cancel非阻塞，task只在worker结束后destroy。
- [x] C3 apps/harmony-controller：正式Stage工程，ArkUI设备列表、增删改、本地非敏感配置保存、连接设置与预检结果；NAPI validateProfile/probeEndpoint/cancel/dispose。单活动probe，每任务ID和代次隔离。ID/中继模式仅验证配置，提示真实会话后端尚未接入。IP按钮明确称为“网络预检”。页面无联网密码输入、无虚假视频或键鼠能力。
- [x] C4 NAPI异步关闭：owner与worker独立引用TSFN，原生任务shared ownership；cancel/dispose标记closing并发出取消，不在UI线程join网络操作。CallJs检查closing丢弃旧事件；env cleanup停止生产；worker完成后释放最后引用并销毁task。正常/失败的payload所有权明确。
- [x] C5 验证：host Rust行为测试（地址/配置、上游帧复用、安全状态与本机loopback服务器探测取消），真实双ABI静态库和HAP构建；不执行真机或视频测试，不依赖模拟成功画面。构建入口显式指定现有DevEco SDK。独立复核后commit。

## 验收与未完成事项

1. 共享核心和HAP在既有SDK24上构建，兼容API22；已有probe不被替换。
2. 错误地址/端口/模式拒绝；IPv4/IPv6标准形式规范化；任何JSON中password等未知字段拒绝，敏感信息不落盘。
3. 即使TCP打开且收到有效protobuf消息也不能进入Active/授予权限；发送字节为零；连接检查结束关闭socket。
4. cancel/dispose不会阻塞UI等待网络，旧事件不影响新请求。
5. profile本地保存包含设备地址及公开信任材料，不包含凭据；加载失败可见且不静默覆盖原记录。
6. 官方完整Session/认证/加密、设备ID协调、中继会话、被控端权限、真实输入、视频和真机均不是本轮完成声明。

## 后续实施顺序

D1 明确 OHOS controller-only target gates（排除 Linux桌面、被控服务及原生媒体）；D2 以官方 Session/Remote 接口接入加密认证消息循环并禁用媒体初始化；D3 受控环境验证ID/中继与安全IP直连；D4 实施被控权限及公网门槛；视频与真机按用户要求后移。新端网络模块不替代官方密码学。
