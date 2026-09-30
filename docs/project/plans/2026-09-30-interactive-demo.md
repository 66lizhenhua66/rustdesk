# 非视频持续会话与 Windows 演示端计划

日期：2026-09-30。基线：5e6c24d5c。用户要求把可实现的继续做出DEMO后再真机验证；视频解码仍后移。本轮交付鸿蒙HAP + Windows演示窗口EXE，真实TCP签名/加密/现场批准/权限/持续会话/受限输入。只影响演示窗口，不注入全桌面输入。

## 交付契约

- 新 Windows `apps/windows-demo-host` 独立crate，复用controller-core上游crypto/protobuf。默认listen 127.0.0.1:21119，允许显式具体LAN IP，拒绝0.0.0.0/::通配；不自动开防火墙/公网端口。
- 每次启动生成临时Ed25519身份与9位peer ID，只导出公开profile JSON；私钥仅内存，不输出。GUI展示可复制公开配置、待审批请求、允许/拒绝、允许输入/撤销、断开和800x450演示画布/文本。关闭即停监听。
- 只允许一个活动/待批准连接。SignedId/v1交换后发随机Hash。DEMO不使用永久密码，空密码请求必须本地批准后才发PeerInfo；拒绝/超时/断开不留权限。请求方ID/name仅作自报提示，不当可信控制设备身份。
- 双方核对6位配对码：SHA256(Hash.challenge UTF8)前4字节大端u32 % 1000000，补零6位。只是人工核对信息，不是远端可提交的授权令牌。
- PeerInfo.platform_additions固定JSON {"ord_demo":1,"input_scope":"demo_window","width":800,"height":450}。持续客户端只接受此demo能力，普通PeerInfo不能打开控制；原单次authenticate保持原行为。
- 登录确认后显式发送所有PermissionInfo=false。GUI批准连接与允许输入是分开的操作；键鼠开关通过Keyboard true/false通知客户端，宿主执行每条输入前再校验当前session/权限，撤权后排队输入也不得执行。
- 只接受MouseEvent(mask=0,无modifiers,x0..799,y0..449)更新演示光标；KeyEvent(seq非空、UTF8<=512字节、无其他按键/修饰字段)更新演示文本，不调用SendInput或执行shell。禁止文件/剪贴板/终端/隧道/提权/摄像头等其他操作。
- 状态回传通过加密Misc.ChatMessage的JSON {"ord_demo_status":1,"x":N,"y":N,"textLength":N}；不回传或持久记录文字正文。客户端仅把它作为反馈，权限只按显式PermissionInfo更新。

## 实施任务

- [x] P1 共享核心增加持久demo连接：controller_connection_create(request,password,handshake_ms)->ControllerSession*；沿用run/cancel/destroy；controller_session_send_pointer(task,x,y)、send_text(task,utf8)返回0排队成功，1未连接，2无权限，3参数无效，4队列满。限定64队列、移动合并；取消/撤权清队列。认证期100..60000ms，活动最长30分钟；不把网络keepalive当用户活动。所有函数调用依然非阻塞UI。
- [x] P2 新demo_host库：公开DemoServer/HostHandle/HostEvent给GUI与测试调用，bind具体地址，签名握手、随机挑战、人工批准、显式权限、输入执行范围/撤权、超时/断开/单连接控制。服务端input状态与画布文本只保存在当前内存，GUI读取snapshot，不把命令无校验排入UI待执行队列。
- [x] P3 Win32 GUI二进制：windows-sys实现窗口和按钮，50ms轮询HostEvent/snapshot；展示坐标、文本、状态与公开连接资料；不用真实全桌面采集/输入。提供明确的启动脚本和导出profile路径。
- [x] P4 鸿蒙NAPI/ArkUI：connectDemo + sendPointer + sendText，显示配对码、等待批准、只读/允许输入、撤权/关闭；800x450无视频操作区、发送文本按钮、手动断开；支持导入公开profile JSON（验证后显式保存，不自动连接，不保存凭据）。旧authenticate与只读probe仍可用。
- [x] P5 测试：保留38项；用真实DemoServer+控制CABI覆盖拒绝、超时、只读不可发送、授予后窗口状态变化、撤权和断线后拒绝、过期approve不能授权新session、错误公钥、不支持demo标记不得active。构建Windows exe与双ABI HAP；模拟器可冒烟，不进行真机或视频测试。
- [x] P6 独立复核、打包DEMO（不含身份私钥或固定密码）、README/进度/报告、commit。

## 后续边界

此Windows程序是限定演示窗口的被控DEMO，不是完成官方RustDesk Windows桌面主程序改造。能够验证真实跨进程加密会话和授权输入，但不宣称屏幕共享或全桌面远控。完整ID/中继、真实画面、账号/可信控制设备、2FA继续会话仍有独立后续任务；本轮不把它们的缺口隐藏为已实现。

## 完成证据

53 项自动化测试、最终双 ABI HAP/EXE 构建、模拟器到 Windows 独立进程操作链路及独立复核已完成。详见 [验证报告](../research/2026-09-30-interactive-demo-validation.md) 和 [使用说明](../DEMO.md)。代码与文档在当前功能分支本地提交，未推送。
