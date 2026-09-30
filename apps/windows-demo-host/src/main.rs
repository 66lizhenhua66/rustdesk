#[cfg(not(windows))]
fn main() {
    eprintln!("This demo host requires Windows.");
}

#[cfg(windows)]
mod windows_gui {
    use std::{ffi::c_void, net::SocketAddr, ptr};
    use windows_demo_host::{DemoServer, HostHandle, HostSnapshot, HostState};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, DrawTextW, Ellipse, EndPaint, FillRect, GetStockObject, InvalidateRect,
            Rectangle, SetBkMode, SetTextColor, DT_NOPREFIX, DT_WORDBREAK, HBRUSH, PAINTSTRUCT,
            TRANSPARENT, WHITE_BRUSH,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::Controls::{EM_SCROLLCARET, EM_SETSEL},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
            KillTimer, PostQuitMessage, RegisterClassW, SendMessageW, SetTimer, SetWindowLongPtrW,
            SetWindowTextW, ShowWindow, TranslateMessage, BS_PUSHBUTTON, CREATESTRUCTW,
            CW_USEDEFAULT, ES_MULTILINE, ES_READONLY, GWLP_USERDATA, HMENU, MSG, SW_SHOW,
            WM_COMMAND, WM_CREATE, WM_DESTROY, WM_NCCREATE, WM_PAINT, WM_TIMER, WNDCLASSW,
            WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
        },
    };

    const PROFILE: i32 = 100;
    const STATUS: i32 = 101;
    const PAIR_CODE: i32 = 102;
    const RECEIVED_TEXT: i32 = 103;
    const ALLOW: i32 = 201;
    const REJECT: i32 = 202;
    const INPUT: i32 = 203;
    const REVOKE: i32 = 204;
    const DISCONNECT: i32 = 205;
    const TIMER: usize = 1;

    struct Ui {
        server: DemoServer,
        handle: HostHandle,
        profile: HWND,
        status: HWND,
        pair_code: HWND,
        received_text: HWND,
        last_status: String,
        last_code: String,
        last_text: String,
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    unsafe fn child(
        parent: HWND,
        class: &str,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        style: u32,
        id: i32,
    ) -> HWND {
        CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            w,
            h,
            parent,
            id as HMENU,
            ptr::null_mut(),
            ptr::null(),
        )
    }
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_NCCREATE {
            let create = &*(lp as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Ui;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let ui = &mut *ptr;
        match msg {
            WM_CREATE => {
                child(
                    hwnd,
                    "STATIC",
                    "Public connection profile (select text to copy)",
                    20,
                    15,
                    700,
                    22,
                    0,
                    0,
                );
                ui.profile = child(
                    hwnd,
                    "EDIT",
                    ui.server.public_profile_json(),
                    20,
                    40,
                    1020,
                    95,
                    WS_BORDER | ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_TABSTOP,
                    PROFILE,
                );
                ui.status = child(hwnd, "STATIC", "Listening", 20, 145, 1020, 30, 0, STATUS);
                ui.pair_code = child(
                    hwnd,
                    "STATIC",
                    "Pair code: -",
                    20,
                    178,
                    1020,
                    22,
                    0,
                    PAIR_CODE,
                );
                for (id, text, x) in [
                    (ALLOW, "Allow connection", 20),
                    (REJECT, "Reject", 195),
                    (INPUT, "Allow input", 335),
                    (REVOKE, "Revoke input", 485),
                    (DISCONNECT, "Disconnect", 650),
                ] {
                    child(
                        hwnd,
                        "BUTTON",
                        text,
                        x,
                        205,
                        160,
                        34,
                        BS_PUSHBUTTON as u32 | WS_TABSTOP,
                        id,
                    );
                }
                child(
                    hwnd,
                    "STATIC",
                    "Demo canvas 800 x 450",
                    20,
                    255,
                    500,
                    22,
                    0,
                    0,
                );
                child(hwnd, "STATIC", "Received text", 840, 255, 200, 22, 0, 0);
                ui.received_text = child(
                    hwnd,
                    "EDIT",
                    "",
                    840,
                    280,
                    200,
                    450,
                    WS_BORDER | ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_TABSTOP,
                    RECEIVED_TEXT,
                );
                SetTimer(hwnd, TIMER, 50, None);
                0
            }
            WM_COMMAND => {
                let id = (wp & 0xffff) as i32;
                if let Some(session) = ui.handle.snapshot().session_id {
                    match id {
                        ALLOW => {
                            ui.handle.approve(session);
                        }
                        REJECT => {
                            ui.handle.reject(session);
                        }
                        INPUT => {
                            ui.handle.allow_input(session);
                        }
                        REVOKE => {
                            ui.handle.revoke_input(session);
                        }
                        DISCONNECT => {
                            ui.handle.disconnect(session);
                        }
                        _ => {}
                    }
                }
                0
            }
            WM_TIMER => {
                while ui.handle.poll_event().is_some() {}
                let snapshot = ui.handle.snapshot();
                let state = match snapshot.state {
                    HostState::Listening => "Listening",
                    HostState::Handshaking => "Secure handshake",
                    HostState::Pending => "Pending local approval",
                    HostState::Active => "Connected",
                };
                let status = format!("{} | {} | session {:?} | requester: {} ({}) | input: {} | position: {},{} | text length: {}", ui.server.local_addr(), state, snapshot.session_id, snapshot.requester_name, snapshot.requester_id, if snapshot.input_allowed { "allowed" } else { "off" }, snapshot.x, snapshot.y, snapshot.text.chars().count());
                if status != ui.last_status {
                    SetWindowTextW(ui.status, wide(&status).as_ptr());
                    ui.last_status = status;
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
                if snapshot.pair_code != ui.last_code {
                    let code = if snapshot.pair_code.is_empty() {
                        "-"
                    } else {
                        &snapshot.pair_code
                    };
                    SetWindowTextW(ui.pair_code, wide(&format!("Pair code: {code}")).as_ptr());
                    ui.last_code = snapshot.pair_code;
                }
                if snapshot.text != ui.last_text {
                    SetWindowTextW(
                        ui.received_text,
                        wide(&snapshot.text.replace('\n', "\r\n")).as_ptr(),
                    );
                    SendMessageW(ui.received_text, EM_SETSEL, usize::MAX, -1);
                    SendMessageW(ui.received_text, EM_SCROLLCARET, 0, 0);
                    ui.last_text = snapshot.text;
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
                0
            }
            WM_PAINT => {
                paint(hwnd, &ui.handle.snapshot());
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, TIMER);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(ptr));
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
    unsafe fn paint(hwnd: HWND, snapshot: &HostSnapshot) {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut ps);
        let rect = windows_sys::Win32::Foundation::RECT {
            left: 20,
            top: 280,
            right: 820,
            bottom: 730,
        };
        FillRect(dc, &rect, GetStockObject(WHITE_BRUSH) as HBRUSH);
        Rectangle(dc, 20, 280, 820, 730);
        Ellipse(
            dc,
            20 + snapshot.x - 5,
            280 + snapshot.y - 5,
            20 + snapshot.x + 5,
            280 + snapshot.y + 5,
        );
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00202020);
        let shown: String = snapshot
            .text
            .chars()
            .rev()
            .take(500)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let text = wide(&shown);
        let mut text_rect = windows_sys::Win32::Foundation::RECT {
            left: 30,
            top: 295,
            right: 810,
            bottom: 440,
        };
        DrawTextW(
            dc,
            text.as_ptr(),
            -1,
            &mut text_rect,
            DT_WORDBREAK | DT_NOPREFIX,
        );
        EndPaint(hwnd, &ps);
    }
    pub fn run() -> Result<(), String> {
        let mut addr: SocketAddr = "127.0.0.1:21119"
            .parse()
            .map_err(|_| "invalid default listen address")?;
        let mut profile_out = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--listen" => {
                    addr = args
                        .next()
                        .ok_or("--listen requires IP:port")?
                        .parse()
                        .map_err(|_| "--listen requires a concrete IP:port")?
                }
                "--profile-out" => {
                    profile_out = Some(args.next().ok_or("--profile-out requires a path")?)
                }
                _ => return Err(format!("unknown option: {arg}")),
            }
        }
        let server = DemoServer::start(addr).map_err(|e| e.to_string())?;
        if let Some(path) = profile_out {
            std::fs::write(path, server.public_profile_json()).map_err(|e| e.to_string())?;
        }
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class = wide("OrdDemoHost");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                hbrBackground: GetStockObject(WHITE_BRUSH) as HBRUSH,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err("window registration failed".into());
            }
            let ui = Box::new(Ui {
                handle: server.handle(),
                server,
                profile: ptr::null_mut(),
                status: ptr::null_mut(),
                pair_code: ptr::null_mut(),
                received_text: ptr::null_mut(),
                last_status: String::new(),
                last_code: String::new(),
                last_text: String::new(),
            });
            let raw = Box::into_raw(ui);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                wide("Open Remote Desk - demo host").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1080,
                815,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                raw as *const c_void,
            );
            if hwnd.is_null() {
                drop(Box::from_raw(raw));
                return Err("window creation failed".into());
            }
            ShowWindow(hwnd, SW_SHOW);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows_gui::run() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR};
        let message: Vec<u16> = error.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "Demo host error".encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_ICONERROR,
            );
        }
    }
}
