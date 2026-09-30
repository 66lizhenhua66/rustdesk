# Windows / HarmonyOS 非视频 DEMO

本轮可以验证：可信被控身份、加密 IP 直连、双方配对码、现场批准、持续连接、单独允许输入、撤权和断开。手机操作区可移动 Windows 演示窗口的光标并发送文字。

这仍是限定演示窗口的工程验证版，没有远程视频、全桌面键鼠、文件传输或公网部署验收。Windows 官方主程序的改造、ID/中继连接、可信控制设备和 2FA 继续流程还在后续计划中。

## 本机构建产物

从仓库根运行：

```powershell
& ./apps/harmony-controller/scripts/package-demo.ps1 -Build
```

产物位于 `apps/harmony-controller/artifacts/interactive-demo/`：Windows EXE、双 ABI（ARM64/x86_64）未签名 HAP、本说明与 SHA256 校验清单。已有本轮构建产物时可省略 `-Build` 仅打包。目录仅用于本地验证，不包含固定密码、签名私钥或设备资料，不提交 Git。分发时还需遵守仓库及依赖的许可义务。

## 在手机和平板上测试

1. 在 DevEco 中打开 `apps/harmony-controller`，连接鸿蒙 6.1 真机，使用自己的调试签名安装运行。未签名 HAP 在模拟器上可安装，不代表真机能直接安装；当前未配置你的签名材料。
2. Windows 与控制设备接入可互通的局域网。用 Windows 实际 LAN IP 启动（以下地址仅示例）：

   ```powershell
   .\windows-demo-host.exe --listen 192.168.1.10:21119 --profile-out .\demo-public-profile.json
   ```

   无参数时只监听 `127.0.0.1:21119`。不接受 `0.0.0.0` / `::` 通配监听，不自动更改防火墙。如果 Windows 阻止连接，仅为该测试程序/端口放行所需私有网络和控制设备来源。
3. 将窗口或文件中的**本次公开 JSON**复制到鸿蒙端“导入公开配置”，点击“验证并预览”，核对来源、公钥和目标地址，再“确认保存”。保存不会发起连接。不要导入不可信来源的公钥。
4. 选择设备，点击“连接演示端”。核对两边的 6 位配对码，在 Windows 点击 **Allow connection**。连接最初是只读；请求方名称/ID 是自报标签，不代表已验证的控制设备身份。
5. 在 Windows 点击 **Allow input**，手机出现“允许输入”后触摸操作区、发送文字，观察 Windows 画布和手机坐标/文本长度反馈。文本限制为 512 UTF-8 字节，只写入演示窗口，手机反馈不回传正文。
6. 点击 **Revoke input**，应立即停止接受操作；点击 **Disconnect** 或手机“断开演示连接”结束会话。离开页面、后台切换也会取消连接。重新连接必须重新批准和单独允许输入。
7. 关闭 Windows 窗口即停止服务。每次重启会重新生成临时 ID 和密钥，原资料会验证失败，应重新导入本次公开配置，不要通过跳过身份校验解决。

批准有时限，连接最长 30 分钟。配对码用于现场人工核对，不能代替可信公钥分发或控制端身份认证。当前没有无人值守功能。

## 模拟器

Windows 保持默认 loopback 监听。用 DevEco 自带 HDC 建立设备到主机的测试转发，然后导入 loopback 公开配置：

```powershell
$hdc = 'G:\Huawei\DevEco Studio\sdk\default\openharmony\toolchains\hdc.exe'
& $hdc -t 127.0.0.1:5555 rport tcp:21119 tcp:21119
& $hdc -t 127.0.0.1:5555 install -r .\harmony-controller-unsigned.hap
```

以上目标是本机模拟器示例；用 `hdc list targets` 核对自己的目标。测试结束用 `hdc fport ls` 找到本次转发，再用 `hdc fport rm <taskstr>` 删除该条。真机 LAN 连接不需要这个转发。

## 测试与反馈

```powershell
cargo test --manifest-path libs/controller-core/Cargo.toml --locked
cargo test --manifest-path apps/windows-demo-host/Cargo.toml --locked
```

真机验证请记录：设备型号、系统版本、LAN/其他网络、是否成功安装、配对码是否一致、批准/只读/允许输入/撤权/重连结果。不要回传密码、签名私钥或完整私有日志。视频、延迟和省流效果尚不能用本 DEMO 验收。
