# 正式入口只读画面计划

基线 `2bdb98094`，用户要求继续对接画面。沿用现有功能分支、审批和原生控制端架构；采用 subagent-driven-development 分模块实现并独立复核，已有用户配置/原型不进入提交。

## 选定方案

首版使用 Windows 主屏 GDI 缩放采集 + 现有 scrap VP8 编码，RustDesk VideoFrame.vp8s 通过已认证的 v1 加密连接发送。鸿蒙 C++ 用固定版本 libvpx 软件解码、NativeWindow/NativeBuffer 显示，图像不经过 ArkTS/JSON。这样先验证模拟器/ARM64 的一致路径；原生 H.264 硬解是后续性能增量，不声称本轮完成。

限制：仅主显示器、保持比例且偶数边长，最大 1280×720、最多约 8fps；相同像素不重复编码发送，VP8 正常帧间编码，不以逐帧 PNG/JPEG 替代视频。键鼠、文件、音频等权限仍 false。锁屏、断开、停止服务结束画面；不采集安全桌面。

## 协议与权限契约

- 本地控制端持久请求 `expectedPeer=secure_video` 明确要求画面；旧 demo / secure_host 路径保持原能力。登录的 `SupportedDecoding.ability_vp8=1` 且 prefer_codec=VP8 表示请求本阶段画面，其他登录选项不解释。
- 被控 `ORD_SECURE_VIDEO=1` 为显式允许本阶段视频的本机配置，默认不启用。远端不能仅凭能力声明自动开启。CM 仍需要逐连接批准；批准前不创建 capturer/encoder，不枚举和发送屏幕，不发送 VideoFrame。
- 成功 PeerInfo 标记 `ord_secure_host=1, media=true, video_codec=vp8, input_scope=none`；displays 仅一个，width/height 为缩放后的图像尺寸（上限1280/720），current_display=0。core 必须校验标记、尺寸、编码、display编号、帧数量/大小；旧 media=false 不能进入画面模式。
- 非视频模式接收到视频拒绝；画面模式在批准前收到视频拒绝。任何 PermissionInfo.enabled=true 仍拒绝。C ABI 输入永不授权。
- 登录/控制包维持64KiB；只有成功确认视频能力后控制端接收上限调至8MiB，单帧VP8≤2MiB，一条VideoFrame最多4帧；不接受其他编码或未知display。首帧必须关键帧，native关键帧头尺寸必须与协商一致。
- 后端压缩帧队列容量1，保持帧顺序，不能随意丢预测帧。断开需使生产者停止并释放DC/bitmap/encoder；慢消费者有界排队，发送超时关闭。用户批准按每次新会话重新获取。

## ABI / Native / UI 契约

新增 `ControllerVideoCallback(const uint8_t *data, uint32_t length, uint32_t width, uint32_t height, int64_t pts, uint8_t key_frame, void *user)`，数据只在回调期间有效。`controller_session_run_video(task, event_callback, video_callback, user)` 供 native 工作线程调用；普通 run 不能在没有视频sink时请求 secure_video。callback同步消耗数据，既有 run/cancel/destroy 保留。

画面模式 connected 事件含 videoWidth/videoHeight/videoCodec，授权标志仍 false。低频 video_status 只带帧计数/累计压缩字节，不带像素。Native decoder 只属于本次 Job，带mutex与明确生命周期；surface销毁/页面退出/切设备即取消会话并销毁图像资源，旧回调不能画入新surface。

NAPI新增 `connectScreen(requestJson, surfaceId: string, timeoutMs, callback): number`。ArkUI先创建XComponent，再用其surfaceId发起连接；surface未就绪不启动。使用 NativeWindow CreateFromSurfaceId，Request/Flush或Abort配对，fence等待有界并close，CPU_READ/CPU_WRITE usage，Map/Unmap配对，DestroyWindow释放引用。官方MCP原文及本机SDK24/API22头文件为依据。

## 任务

- [x] V1：Windows 新 secure_video 模块负责缩放采集/VP8编码/容量1生产者；只在本机批准后从正式分支薄接入；保留非媒体路径。新增采集尺寸、编码可解码、队列取消/审批前不启动等针对性测试。
- [x] V2：共享核心增加明确 video 模式、二进制回调、能力/包大小/首帧/权限门禁；回归原模式，测试批准前视频、错codec/尺寸/display、无sink、取消和有效视频。
- [x] V3：构建固定libvpx 1.15.2双OHOS ABI，新增独立VideoRenderer类，软件解码+NativeWindow，生命周期/畸形帧拒绝；记录来源/哈希/许可证，不改全局Cargo缓存。
- [x] V4：NAPI和ArkUI绑定XComponent、连接、低频统计、取消清理及“仅查看”提示；模拟器用受控VP8样例检验真实解码/显示，Windows/HAP构建验证。
- [x] V5：独立安全/生命周期复核；真实CM批准后查看主屏，取消/再连验证；原始桌面截图不进Git。若需本机用户点击批准，准备好完整结果后交还该一步。更新计划、进度、报告并本地提交。

本轮不把真机、GPU硬解、画质/延迟/省流性能、公网与输入视为已验收。捕获失败必须显示错误/断开，不能用静态截图或测试色块冒充真实桌面链路。
