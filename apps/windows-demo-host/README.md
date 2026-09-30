# Windows demo host

This isolated Win32 program accepts one encrypted demo session at a time. It shows a local 800 x 450 canvas. Approved remote pointer and text events update only that canvas; they do not operate the Windows desktop, clipboard, files, terminal, or other applications.

Build on Windows with `powershell -File scripts/build.ps1`. Start with `powershell -File scripts/run.ps1`. The default listener is `127.0.0.1:21119`, so a controller on another device needs an explicit LAN address:

```powershell
powershell -File scripts/run.ps1 -Listen 192.168.1.20:21119 -ProfileOut .\demo-profile.json
```

`--listen` takes a literal, concrete IP and port; wildcard addresses are refused. `--profile-out` writes only the public connection profile. The peer ID and Ed25519 signing identity are generated on every start. The signing secret stays in process memory and is never written to the profile.

The window displays the public profile, the self-reported requester, and the six-digit pairing code. Compare the code with the controller before selecting **Allow connection**. This grants a read-only session. **Allow input** separately enables demo pointer/text updates; **Revoke input** immediately closes the host execution gate. **Reject** and **Disconnect** end the current session. Closing the window stops the listener.

The network connection uses SignedId/v1 and encrypted RustDesk messages from `remote_controller_core`. Only pointer movement and text sequence messages are accepted as demo input. The encrypted status response contains coordinates and text length, never the text content.
