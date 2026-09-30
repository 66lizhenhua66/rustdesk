# 严格身份握手与非媒体登录实施计划

日期：2026-09-30。基线：6a96b474a。用户授权继续下一步并补测试；视频解码和真机后移。本轮从“零发送预检”推进到可测试的严格身份握手与非媒体登录，但不把上游宽松IP入口或TCP可达当成安全会话。

## 接入边界

完整Root crate依赖桌面/被控/媒体；本轮在controller-core编译可独立的上游身份函数和TCP加密实现，原始src/common.rs、hbb_common/tcp.rs保持不变。构建时核对规范化SHA后提取精确源码片段到OUT_DIR，不手工维护第二份密码学实现；上游变动时fail closed要求复核。libsodium使用固定来源和构建脚本，sodiumoxide仍为同类上游依赖。

## 任务

- [x] A1 固定libsodium源码/构建来源，主机+OHOS x86_64/ARM64可编译；不提交生成的库和SDK。
- [x] A2 复用上游SignedId校验、临时密钥封装、Encrypt v0/v1及其固定向量测试；验证公钥错误、签名篡改、版本/transcript错配和短密文拒绝。
- [x] A3 非媒体认证：新增独立Session C ABI（不替换probe），输入literal endpoint、期望peerId、预先可信的Ed25519 peerPublicKey（32B Base64）、可选SHA256指纹与本次密码。密钥缺失就不连接；首包不是有效SignedId立即拒绝，验证peerId后协商加密，再读Hash，按上游SHA256(SHA256(password||salt)||challenge)发LoginRequest。密码只驻留本次内存，不放profile/日志。禁用键鼠/剪贴板/音频/文件/摄像头；收到PeerInfo仅报告authenticated，所有操作权限仍false，并立即发送关闭原因结束本轮无媒体登录验证。2FA/审批未满足保持未授权并明确需要进一步交互，不假装成功。
- [x] A4 鸿蒙桥接/页面：在原只读预检之外增加“验证加密登录”的显式入口，仅direct可用；新增公开peerId/peerPublicKey配置（老记录可加载为draft），本次密码输入不保存。取消非阻塞、事件代次隔离，证书/签名失败不降级，不显示远控Active。
- [x] A5 用受控loopback对端测试真实wire：成功签名/解密/挑战/登录响应、错误密钥/ID/明文首包/密文破坏、拒绝登录、超时、取消；敏感数据和Active不可提前出现。测试对端用原始协议/同库构造消息，独立固定向量验证crypto。保留全部预检回归测试。
- [x] A6 双ABI+HAP构建、独立安全与生命周期复核、更新文档和commit。不连接真实Windows电脑、不执行真机或视频验证；实际官方被控互通是后续明确验收。

## 兼容性约束

当前原版direct_server传secure=false，严格新入口必须拒绝其明文；不能仅因为“直接IP”忽略身份。受控测试对端提供官方SignedId/加密路径，不能宣称所有官方IP部署已兼容。ID/中继建立通道、官方完整Session/Remote移植、可操作权限、2FA继续会话与媒体仍有后续任务。

## 关闭与资源

新Session不在UI线程执行网络。read/write有总deadline与分段timeout，取消能关闭已连接socket；连接中的系统调用有有限超时。native task只在worker返回后销毁，TSFN owner/worker引用平衡。密码缓冲通过zeroize清理；不声称JS输入字符串能强制立即擦除。
