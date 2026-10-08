# 键盘、指针与即时画质设置验证记录

日期：2026-10-09。基线：`8d025efbc`。需求与阶段见 [实施计划](../plans/2026-10-09-input-and-video-controls.md)。

## 已完成行为

- 独立键盘悬浮按钮与工具浮球成组定位；按实际固定态键盘区域计算上方视口，浮窗键盘不贡献停靠占用。保留整屏背景与键盘 NONE 系统模式，应用自己计算视口，避免系统和应用重复缩放。
- 指针模式修复像素差与归一化相对量的错配。启用后将真实鼠标移到可见画面中心，滑动累计目标与点击/滚轮/拖动统一；真实指针通过下一视频帧显示，无本地双光标。未获准控制时不会移动远端鼠标。
- 普通连接默认标准1080p/15FPS；省流720p、标准1080p、高清1440p，FPS可选10/15/30。会话内可申请即时切换，等待被控端确认后更新实际档位和尺寸。右侧浮球菜单提供「画质 / FPS」，工作台偏好设置可选择下次默认值。
- Windows与鸿蒙软编解码链路均扩大到1440p上限；分辨率切换要求新关键帧，按合法尺寸重新初始化解码器。实际FPS不是固定性能承诺；静止桌面仍避免重复编码。

## 官方键盘依据

通过 HarmonyOS developer knowledge MCP 检索并读取以下资料：

- [AvoidAreaType](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-window-e#avoidareatype7)：TYPE_KEYBOARD（API9）明确为固定态软键盘区域。
- [窗口 FAQ](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/window-faqs#如何获取软键盘高度)：使用 getWindowAvoidArea(TYPE_KEYBOARD).bottomRect 与 avoidAreaChange 获取固定键盘占用。
- [Window](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/arkts-apis-window-window#onkeyboardheightchange7)：keyboardHeightChange仅为固定键盘高度，px单位；未额外引入重复事件监听。

普通输入客户端没有采用输入法服务端的 PanelFlag 接口，也不根据 keyboardOpen 推测浮窗位置。当前先完成鸿蒙适配，跨平台契约保持「键盘开关状态」与「实际占用矩形」独立。

## 视频协议与安全边界

仅显式 opt-in 的普通 secure_control 登录携带 OrdVideoSettings（版本1，请求1），服务端批准后返回6字段能力元数据并先发 OrdVideoState，再发首帧。运行期最多一个等待确认请求，序号从2递增，确认严格匹配版本、序号、档位、FPS、偶数尺寸与上限；5秒未确认则明确失败关闭。

host停旧worker并丢弃其receiver，再确认并安装新worker；客户端在确认前仍按旧尺寸解码，确认后强制新关键帧。新消息使用fixed64请求序号，最小正文21字节以上，不放宽原16字节短明文拒绝策略。参数不能开启输入、剪贴板、文件或其他通道。旧请求与可信只读路径保留720p/8FPS和原元数据；hbb_common子模块未修改。

## 验证证据

| 范围 | 结果 |
| --- | --- |
| controller-core | 142项通过，包括新增指针坐标、视频参数/确认/换尺寸关键帧、错误与超时拒绝 |
| Harmony Node | 38项通过，包括停靠/浮窗几何、悬浮组、配置确认投影与默认值 |
| Windows专属视频 | 6项通过，包括三档及高清切回省流的真实VP8首关键帧编码/解码与尺寸检查 |
| Windows批准门禁 | 1项通过；host策略等14项也包含在core回归中，不重复计总数 |
| OHOS | x86_64/aarch64 release编译成功，Hvigor HAP构建成功 |
| Windows Debug | Rust库和Flutter整包构建成功，已恢复`192.168.10.3:21120`、仅允许测试手机来源 |
| 签名 | 原调试身份verify-app、codesign、verify-profile通过，设备绑定匹配，签名配置未变 |

core默认并发曾出现既有Windows回环10053错误，单项与完整单线程重跑通过；本轮按单线程完整验证。未加入无关重试机制。构建保留既有dead_code/IME异常处理等警告。

手机产物位于忽略目录 `apps/harmony-controller/artifacts/input-video-20261009/entry-default-signed.hap`。原无线调试地址随后拒绝连接，安装未成功；等待用户重新提供可用调试地址。没有自动操作手机或批准会话，继续按用户要求由其自行验收。

最终签名包 SHA-256：`9da41864282a50722ea144558ebe39077eba1745fb2bf43cb860256c25189386`。

## 待验范围

- 手机安装及停靠/浮窗输入法实际切换、键盘按钮收起与位置恢复。
- 真实指针出现、连续触摸板移动及点击/拖动一致性。
- 真机多档分辨率/FPS连续切换与清晰度、CPU、延迟；模型和协议fixture不能替代实际NativeWindow呈现。

当前悬浮双按钮组需要至少120 vp加上下安全区的有效高度；极小窗口不足时底部按钮可能部分被键盘遮住，保留为待设备验收的布局限制。画质Select的系统下拉层可能导致画布失焦并收起键盘，继续保留原有失焦释放，不屏蔽安全清理。

## 回归面

新增能力集中在controller-core视频设置模块、Windows secure_video_settings策略与专属安全视频入口；session、proto与NAPI仅接入新协商/确认。原生渲染器仅允许经核心确认的新尺寸关键帧重建。鸿蒙改变固定键盘区域来源标记、有效viewport和悬浮布局，并新增画质偏好与实际回执显示。共享指针核心修复坐标输出，其他平台也复用该算法；未修改系统输入授权或普通上游远控服务。用户本地签名、构建脚本及SSH改动保留。
