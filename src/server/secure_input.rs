use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    virtual_width: i32,
    virtual_height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Held {
    Button(u8),
    Key(u16, bool),
    Unicode(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Move(i32, i32),
    RelativeMove(i32, i32),
    Wheel(i32, i32),
    Press(Held, bool),
}

enum Command {
    Move(u32, u32),
    RelativeMove(i32, i32),
    Button(u8, bool),
    Wheel(i32, i32),
    Key(String, bool),
    Text(String),
    ReleaseAll,
    KeepAlive,
}

trait Injector {
    fn geometry(&self) -> Result<Geometry, String>;
    fn send(&mut self, actions: &[Action]) -> usize;
}

struct Session<'a, I: Injector> {
    id: i32,
    supported: bool,
    owner: &'a AtomicI32,
    injector: I,
    token: Option<[u8; 16]>,
    geometry: Option<Geometry>,
    held: BTreeSet<Held>,
    poisoned: bool,
    last_activity: Instant,
}

impl<'a, I: Injector> Session<'a, I> {
    fn expire(&mut self, now: Instant) -> Result<bool, String> {
        if self.token.is_some()
            && now.saturating_duration_since(self.last_activity) >= Duration::from_secs(3)
        {
            self.revoke()?;
            return Ok(true);
        }
        Ok(false)
    }
    fn new(id: i32, supported: bool, owner: &'a AtomicI32, injector: I) -> Self {
        Self {
            id,
            supported,
            owner,
            injector,
            token: None,
            geometry: None,
            held: BTreeSet::new(),
            poisoned: false,
            last_activity: Instant::now(),
        }
    }
    fn grant(&mut self, token: [u8; 16]) -> Result<(), String> {
        if !self.supported || self.poisoned || self.id <= 0 {
            return Err("Input is unavailable".into());
        }
        if self.token.is_some() {
            return Ok(());
        }
        let geometry = self.injector.geometry()?;
        self.owner
            .compare_exchange(0, self.id, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Another input owner or failed release blocks authorization".to_owned())?;
        self.geometry = Some(geometry);
        self.token = Some(token);
        self.last_activity = Instant::now();
        Ok(())
    }

    fn apply(&mut self, token: &[u8], command: Command) -> Result<bool, String> {
        if !self.accepts(token) {
            return Ok(false);
        }
        let geometry = self.injector.geometry()?;
        if self.geometry != Some(geometry) {
            return Err("Primary display geometry changed".into());
        }
        self.last_activity = Instant::now();
        if matches!(command, Command::KeepAlive) {
            return Ok(true);
        }
        if matches!(command, Command::ReleaseAll) {
            if self.release_held().is_err() {
                self.poison();
                return Err("Input release failed".into());
            }
            return Ok(true);
        }
        let actions = match command {
            Command::Move(x, y) => vec![Action::Move(
                absolute_coordinate(x, geometry.width, geometry.left, geometry.virtual_width)?,
                absolute_coordinate(y, geometry.height, geometry.top, geometry.virtual_height)?,
            )],
            Command::RelativeMove(dx, dy)
                if (-65535..=65535).contains(&dx) && (-65535..=65535).contains(&dy) =>
            {
                vec![Action::RelativeMove(dx, dy)]
            }
            Command::Button(button, down) if button <= 2 => {
                let key = Held::Button(button);
                if !down && !self.held.contains(&key) {
                    return Ok(false);
                }
                vec![Action::Press(key, down)]
            }
            Command::Wheel(dx, dy) if (-10..=10).contains(&dx) && (-10..=10).contains(&dy) => {
                let mut actions = Vec::new();
                if dx != 0 {
                    actions.push(Action::Wheel(dx, 0));
                }
                if dy != 0 {
                    actions.push(Action::Wheel(0, dy));
                }
                actions
            }
            Command::Key(code, down) => {
                let key = semantic_key(&code).ok_or("Unsupported input key")?;
                if !down && !self.held.contains(&key) {
                    return Ok(false);
                }
                vec![Action::Press(key, down)]
            }
            Command::Text(text) if !text.is_empty() && text.len() <= 512 => text
                .encode_utf16()
                .flat_map(|unit| {
                    [
                        Action::Press(Held::Unicode(unit), true),
                        Action::Press(Held::Unicode(unit), false),
                    ]
                })
                .collect(),
            _ => return Err("Invalid input command".into()),
        };
        self.dispatch(&actions)?;
        Ok(true)
    }

    fn accepts(&self, token: &[u8]) -> bool {
        self.supported
            && !self.poisoned
            && self.owner.load(Ordering::SeqCst) == self.id
            && self.token.as_ref().is_some_and(|expected| {
                token.len() == expected.len()
                    && expected
                        .iter()
                        .zip(token)
                        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                        == 0
            })
    }

    fn poison(&mut self) {
        self.token = None;
        self.poisoned = true;
        self.supported = false;
    }

    fn dispatch(&mut self, actions: &[Action]) -> Result<(), String> {
        let accepted = self.injector.send(actions).min(actions.len());
        for action in &actions[..accepted] {
            if let Action::Press(key, down) = action {
                if *down {
                    self.held.insert(*key);
                } else {
                    self.held.remove(key);
                }
            }
        }
        if accepted != actions.len() {
            return Err("Windows did not accept all input events".into());
        }
        Ok(())
    }

    fn release_held(&mut self) -> Result<(), String> {
        if self.held.is_empty() {
            return Ok(());
        }
        self.injector.geometry()?;
        let actions: Vec<_> = self
            .held
            .iter()
            .map(|key| Action::Press(*key, false))
            .collect();
        self.dispatch(&actions)
    }

    fn revoke(&mut self) -> Result<(), String> {
        self.token = None;
        if self.poisoned {
            return Err("Input release failed".into());
        }
        if self.release_held().is_err() {
            self.poison();
            return Err("Input release failed".into());
        }
        self.geometry = None;
        if self.owner.load(Ordering::SeqCst) == self.id {
            self.owner
                .compare_exchange(self.id, 0, Ordering::SeqCst, Ordering::SeqCst)
                .map_err(|_| "Input owner changed during cleanup".to_owned())?;
        }
        Ok(())
    }
}

impl<I: Injector> Drop for Session<'_, I> {
    fn drop(&mut self) {
        if !self.poisoned {
            if let Err(error) = self.revoke() {
                #[cfg(feature = "ord-secure-host")]
                hbb_common::log::error!("Secure input cleanup failed: {error}");
                #[cfg(not(feature = "ord-secure-host"))]
                eprintln!("Secure input cleanup failed: {error}");
            }
        }
    }
}

fn absolute_coordinate(value: u32, primary: i32, origin: i32, extent: i32) -> Result<i32, String> {
    if value > 65535 || primary < 2 || extent < 2 {
        return Err("Invalid primary screen coordinates".into());
    }
    let pixel = i64::from(value) * i64::from(primary - 1) / 65535;
    let position = pixel - i64::from(origin);
    if position < 0 || position >= i64::from(extent) {
        return Err("Primary screen is outside the desktop".into());
    }
    Ok((position * 65535 / i64::from(extent - 1)) as i32)
}

fn semantic_key(code: &str) -> Option<Held> {
    let (scan, extended) = if code.len() == 4 && code.starts_with("Key") {
        let scan = match code.as_bytes()[3] {
            b'A' => 0x1E,
            b'B' => 0x30,
            b'C' => 0x2E,
            b'D' => 0x20,
            b'E' => 0x12,
            b'F' => 0x21,
            b'G' => 0x22,
            b'H' => 0x23,
            b'I' => 0x17,
            b'J' => 0x24,
            b'K' => 0x25,
            b'L' => 0x26,
            b'M' => 0x32,
            b'N' => 0x31,
            b'O' => 0x18,
            b'P' => 0x19,
            b'Q' => 0x10,
            b'R' => 0x13,
            b'S' => 0x1F,
            b'T' => 0x14,
            b'U' => 0x16,
            b'V' => 0x2F,
            b'W' => 0x11,
            b'X' => 0x2D,
            b'Y' => 0x15,
            b'Z' => 0x2C,
            _ => return None,
        };
        (scan, false)
    } else if code.len() == 6 && code.starts_with("Digit") {
        let scan = match code.as_bytes()[5] {
            b'0' => 0x0B,
            b'1' => 0x02,
            b'2' => 0x03,
            b'3' => 0x04,
            b'4' => 0x05,
            b'5' => 0x06,
            b'6' => 0x07,
            b'7' => 0x08,
            b'8' => 0x09,
            b'9' => 0x0A,
            _ => return None,
        };
        (scan, false)
    } else if code.len() == 7 && code.starts_with("Numpad") {
        let scan = match code.as_bytes()[6] {
            b'0' => 0x52,
            b'1' => 0x4F,
            b'2' => 0x50,
            b'3' => 0x51,
            b'4' => 0x4B,
            b'5' => 0x4C,
            b'6' => 0x4D,
            b'7' => 0x47,
            b'8' => 0x48,
            b'9' => 0x49,
            _ => return None,
        };
        (scan, false)
    } else {
        match code {
            "Backspace" => (0x0E, false),
            "Tab" => (0x0F, false),
            "Enter" => (0x1C, false),
            "Escape" => (0x01, false),
            "Space" => (0x39, false),
            "PageUp" => (0x49, true),
            "PageDown" => (0x51, true),
            "End" => (0x4F, true),
            "Home" => (0x47, true),
            "ArrowLeft" => (0x4B, true),
            "ArrowUp" => (0x48, true),
            "ArrowRight" => (0x4D, true),
            "ArrowDown" => (0x50, true),
            "Delete" => (0x53, true),
            "Shift" => (0x2A, false),
            "ShiftLeft" => (0x2A, false),
            "ShiftRight" => (0x36, false),
            "Control" => (0x1D, false),
            "ControlLeft" => (0x1D, false),
            "ControlRight" => (0x1D, true),
            "Alt" => (0x38, false),
            "AltLeft" => (0x38, false),
            "AltRight" => (0x38, true),
            "F1" => (0x3B, false),
            "F2" => (0x3C, false),
            "F3" => (0x3D, false),
            "F4" => (0x3E, false),
            "F5" => (0x3F, false),
            "F6" => (0x40, false),
            "F7" => (0x41, false),
            "F8" => (0x42, false),
            "F9" => (0x43, false),
            "F10" => (0x44, false),
            "F11" => (0x57, false),
            "F12" => (0x58, false),
            "Minus" => (0x0C, false),
            "Equal" => (0x0D, false),
            "BracketLeft" => (0x1A, false),
            "BracketRight" => (0x1B, false),
            "Backslash" => (0x2B, false),
            "Semicolon" => (0x27, false),
            "Quote" => (0x28, false),
            "Backquote" => (0x29, false),
            "Comma" => (0x33, false),
            "Period" => (0x34, false),
            "Slash" => (0x35, false),
            "CapsLock" => (0x3A, false),
            "Insert" => (0x52, true),
            "NumLock" => (0x45, true),
            "ScrollLock" => (0x46, false),
            "MetaLeft" => (0x5B, true),
            "MetaRight" => (0x5C, true),
            "ContextMenu" => (0x5D, true),
            "PrintScreen" => (0x37, true),
            "NumpadAdd" => (0x4E, false),
            "NumpadSubtract" => (0x4A, false),
            "NumpadMultiply" => (0x37, false),
            "NumpadDivide" => (0x35, true),
            "NumpadDecimal" => (0x53, false),
            "NumpadEnter" => (0x1C, true),
            _ => return None,
        }
    };
    Some(Held::Key(scan, extended))
}

fn consume_request_budget(remaining: &mut usize, enabled: bool) -> Result<(), String> {
    if enabled {
        *remaining = remaining
            .checked_sub(1)
            .ok_or("Input rate limit exceeded")?;
    }
    Ok(())
}

#[cfg(feature = "ord-secure-host")]
pub use runtime::Control;

#[cfg(feature = "ord-secure-host")]
mod runtime {
    use super::*;
    use base::message_proto::{ord_input_event, Message, OrdInputEvent, OrdInputState};
    use hbb_common::sodiumoxide;
    use winapi::um::{
        wingdi::DeleteDC,
        winuser::{
            GetSystemMetrics, SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
            KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
            MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
            MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
            MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
            SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        },
    };

    static INPUT_OWNER: AtomicI32 = AtomicI32::new(0);
    const MAX_ACTIONS_PER_SECOND: usize = 2048;

    pub struct Control {
        session: Session<'static, WindowsInjector>,
        remaining: usize,
    }

    impl Control {
        pub fn new(id: i32) -> Self {
            Self {
                session: Session::new(id, false, &INPUT_OWNER, WindowsInjector),
                remaining: MAX_ACTIONS_PER_SECOND,
            }
        }

        pub fn configure(&mut self, negotiated: bool) {
            self.session.supported = negotiated && local_allowed() && !self.session.poisoned;
        }

        pub fn supported(&self) -> bool {
            self.session.supported
        }
        pub fn enabled(&self) -> bool {
            self.session.token.is_some()
        }
        pub fn display_change_allowed(&self) -> bool {
            self.enabled() && local_allowed()
        }
        pub fn display_capability_allowed() -> bool {
            local_allowed()
        }
        pub fn poisoned(&self) -> bool {
            self.session.poisoned
        }

        pub fn grant(&mut self) -> Result<(), String> {
            if !local_allowed() {
                return Err("Input is disabled locally".into());
            }
            if self.enabled() {
                return Ok(());
            }
            sodiumoxide::init().map_err(|_| "Input token generator unavailable")?;
            let mut token = [0; 16];
            sodiumoxide::randombytes::randombytes_into(&mut token);
            self.session.grant(token)
        }

        pub fn revoke(&mut self) -> Result<(), String> {
            self.session.revoke()
        }

        pub fn request_enabled(&mut self, enabled: bool) -> Result<(), String> {
            if !enabled {
                return self.revoke();
            }
            let result =
                consume_request_budget(&mut self.remaining, enabled).and_then(|_| self.grant());
            if let Err(error) = result {
                self.revoke()?;
                return Err(error);
            }
            Ok(())
        }

        pub fn tick(&mut self) -> Result<bool, String> {
            self.remaining = MAX_ACTIONS_PER_SECOND;
            if self.enabled() && !local_allowed() {
                self.session.revoke()?;
                self.session.supported = false;
                return Ok(true);
            }
            self.session.expire(Instant::now())
        }

        pub fn state(&self, request_id: u64) -> Message {
            let mut message = Message::new();
            message.set_ord_input_state(OrdInputState {
                version: 1,
                supported: self.supported(),
                enabled: self.enabled(),
                request_id,
                grant_token: self
                    .session
                    .token
                    .map(|token| token.to_vec())
                    .unwrap_or_default()
                    .into(),
                ..Default::default()
            });
            message
        }

        pub fn apply(&mut self, event: OrdInputEvent) -> Result<bool, String> {
            if !self.session.accepts(&event.grant_token) {
                return Ok(false);
            }
            if !local_allowed() {
                return Err("Input is disabled locally".into());
            }
            if event.version != 1
                || event
                    .special_fields
                    .unknown_fields()
                    .iter()
                    .next()
                    .is_some()
            {
                return Err("Invalid input version or fields".into());
            }
            let command = match event.command {
                Some(ord_input_event::Command::Move(value)) if clean(&value.special_fields) => {
                    Command::Move(value.x, value.y)
                }
                Some(ord_input_event::Command::MoveRelative(value))
                    if clean(&value.special_fields) =>
                {
                    Command::RelativeMove(value.dx, value.dy)
                }
                Some(ord_input_event::Command::Button(value)) if clean(&value.special_fields) => {
                    Command::Button(
                        value
                            .button
                            .enum_value()
                            .map_err(|_| "Unknown input button")? as u8,
                        value.down,
                    )
                }
                Some(ord_input_event::Command::Wheel(value)) if clean(&value.special_fields) => {
                    Command::Wheel(value.dx, value.dy)
                }
                Some(ord_input_event::Command::Key(value)) if clean(&value.special_fields) => {
                    Command::Key(value.code, value.down)
                }
                Some(ord_input_event::Command::Text(value)) if clean(&value.special_fields) => {
                    Command::Text(value.text)
                }
                Some(ord_input_event::Command::ReleaseAll(value))
                    if clean(&value.special_fields) =>
                {
                    Command::ReleaseAll
                }
                Some(ord_input_event::Command::KeepAlive(value))
                    if clean(&value.special_fields) =>
                {
                    Command::KeepAlive
                }
                _ => return Err("Unknown input command or fields".into()),
            };
            let cost = match &command {
                Command::Text(text) => text.len().saturating_mul(2),
                Command::ReleaseAll => 0,
                Command::Wheel(..) => 2,
                _ => 1,
            };
            self.remaining = self
                .remaining
                .checked_sub(cost)
                .ok_or("Input rate limit exceeded")?;
            self.session.apply(&event.grant_token, command)
        }
    }

    fn clean(fields: &hbb_common::protobuf::SpecialFields) -> bool {
        fields.unknown_fields().iter().next().is_none()
    }

    fn local_allowed() -> bool {
        std::env::var("ORD_SECURE_INPUT").ok().as_deref() == Some("1")
            && crate::server::Connection::is_permission_enabled_locally(
                base::config::keys::OPTION_ENABLE_KEYBOARD,
            )
    }

    struct WindowsInjector;

    impl Injector for WindowsInjector {
        fn geometry(&self) -> Result<Geometry, String> {
            crate::server::secure_video::visible_desktop().map_err(|error| error.to_string())?;
            let (dc, width, height) =
                crate::server::secure_video::primary_dc().map_err(|error| error.to_string())?;
            unsafe {
                DeleteDC(dc);
                let geometry = Geometry {
                    width: width as i32,
                    height: height as i32,
                    left: GetSystemMetrics(SM_XVIRTUALSCREEN),
                    top: GetSystemMetrics(SM_YVIRTUALSCREEN),
                    virtual_width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    virtual_height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
                };
                if geometry.width != GetSystemMetrics(SM_CXSCREEN)
                    || geometry.height != GetSystemMetrics(SM_CYSCREEN)
                    || geometry.virtual_width < geometry.width
                    || geometry.virtual_height < geometry.height
                {
                    return Err("Primary display input geometry is inconsistent".into());
                }
                Ok(geometry)
            }
        }

        fn send(&mut self, actions: &[Action]) -> usize {
            if actions.is_empty() {
                return 0;
            }
            let mut inputs: Vec<INPUT> = actions.iter().map(native_input).collect();
            if crate::server::secure_video::visible_desktop().is_err() {
                return 0;
            }
            // Releases carry the same marker and click guard as normal injection.
            let now = hbb_common::get_time();
            crate::server::CLICK_TIME.store(now, Ordering::SeqCst);
            crate::server::MOUSE_MOVE_TIME.store(now, Ordering::SeqCst);
            unsafe {
                SendInput(
                    inputs.len() as _,
                    inputs.as_mut_ptr(),
                    std::mem::size_of::<INPUT>() as _,
                ) as usize
            }
        }
    }

    pub(super) fn native_input(action: &Action) -> INPUT {
        let mut input: INPUT = unsafe { std::mem::zeroed() };
        unsafe {
            match action {
                Action::Press(Held::Key(key, extended), down) => {
                    input.type_ = INPUT_KEYBOARD;
                    *input.u.ki_mut() = KEYBDINPUT {
                        wVk: 0,
                        wScan: *key,
                        dwFlags: KEYEVENTF_SCANCODE
                            | (if *extended { KEYEVENTF_EXTENDEDKEY } else { 0 })
                            | (if *down { 0 } else { KEYEVENTF_KEYUP }),
                        time: 0,
                        dwExtraInfo: enigo::ENIGO_INPUT_EXTRA_VALUE,
                    };
                }
                Action::Press(Held::Unicode(unit), down) => {
                    input.type_ = INPUT_KEYBOARD;
                    *input.u.ki_mut() = KEYBDINPUT {
                        wVk: 0,
                        wScan: *unit,
                        dwFlags: KEYEVENTF_UNICODE | (if *down { 0 } else { KEYEVENTF_KEYUP }),
                        time: 0,
                        dwExtraInfo: enigo::ENIGO_INPUT_EXTRA_VALUE,
                    };
                }
                _ => {
                    input.type_ = INPUT_MOUSE;
                    let (dx, dy, flags, data) = match action {
                        Action::Move(x, y) => (
                            *x,
                            *y,
                            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                            0,
                        ),
                        Action::RelativeMove(x, y) => (*x, *y, MOUSEEVENTF_MOVE, 0),
                        Action::Wheel(x, y) => (
                            0,
                            0,
                            if *x != 0 {
                                MOUSEEVENTF_HWHEEL
                            } else {
                                MOUSEEVENTF_WHEEL
                            },
                            (if *x != 0 { *x * 120 } else { *y * 120 }) as u32,
                        ),
                        Action::Press(Held::Button(button), down) => (
                            0,
                            0,
                            match (*button, *down) {
                                (0, true) => MOUSEEVENTF_LEFTDOWN,
                                (0, false) => MOUSEEVENTF_LEFTUP,
                                (1, true) => MOUSEEVENTF_RIGHTDOWN,
                                (1, false) => MOUSEEVENTF_RIGHTUP,
                                (2, true) => MOUSEEVENTF_MIDDLEDOWN,
                                _ => MOUSEEVENTF_MIDDLEUP,
                            },
                            0,
                        ),
                        _ => unreachable!(),
                    };
                    *input.u.mi_mut() = MOUSEINPUT {
                        dx,
                        dy,
                        mouseData: data,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: enigo::ENIGO_INPUT_EXTRA_VALUE,
                    };
                }
            }
        }
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn disabling_input_is_available_after_request_budget_is_exhausted() {
        let mut remaining = 1;
        assert!(consume_request_budget(&mut remaining, true).is_ok());
        assert!(consume_request_budget(&mut remaining, true).is_err());
        assert!(consume_request_budget(&mut remaining, false).is_ok());
    }

    struct FakeInjector {
        events: Rc<RefCell<Vec<Action>>>,
        fail: bool,
        ready: bool,
    }
    impl Injector for FakeInjector {
        fn geometry(&self) -> Result<Geometry, String> {
            if !self.ready {
                return Err("Desktop unavailable".into());
            }
            Ok(Geometry {
                width: 1920,
                height: 1080,
                left: -1280,
                top: 0,
                virtual_width: 3200,
                virtual_height: 1080,
            })
        }
        fn send(&mut self, actions: &[Action]) -> usize {
            if self.fail {
                return 0;
            }
            self.events.borrow_mut().extend_from_slice(actions);
            actions.len()
        }
    }
    fn injector(events: &Rc<RefCell<Vec<Action>>>) -> FakeInjector {
        FakeInjector {
            events: events.clone(),
            fail: false,
            ready: true,
        }
    }

    #[test]
    fn local_gate_and_fresh_grant_are_required_for_each_event() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = Session::new(1, false, &owner, injector(&events));
        assert!(session.grant([1; 16]).is_err());
        assert!(!session.apply(&[1; 16], Command::Move(0, 0)).unwrap());
        session.supported = true;
        session.grant([1; 16]).unwrap();
        assert!(session
            .apply(&[1; 16], Command::Move(65535, 65535))
            .unwrap());
        session.revoke().unwrap();
        session.grant([2; 16]).unwrap();
        assert!(!session.apply(&[1; 16], Command::Move(0, 0)).unwrap());
        assert_eq!(events.borrow().len(), 1);
    }

    #[test]
    fn revoke_and_drop_release_owned_keys_and_buttons_before_lease_reuse() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        {
            let mut session = Session::new(1, true, &owner, injector(&events));
            session.grant([1; 16]).unwrap();
            session
                .apply(&[1; 16], Command::Key("Control".into(), true))
                .unwrap();
            session.apply(&[1; 16], Command::Button(0, true)).unwrap();
            session.revoke().unwrap();
            assert_eq!(owner.load(Ordering::SeqCst), 0);
            assert!(events
                .borrow()
                .contains(&Action::Press(Held::Key(0x1D, false), false)));
            assert!(events
                .borrow()
                .contains(&Action::Press(Held::Button(0), false)));
            session.grant([2; 16]).unwrap();
            session
                .apply(&[2; 16], Command::Key("KeyA".into(), true))
                .unwrap();
        }
        assert!(events
            .borrow()
            .contains(&Action::Press(Held::Key(0x1E, false), false)));
        assert_eq!(owner.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unsafe_desktop_and_failed_release_fail_closed_without_unlocking_owner() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = Session::new(1, true, &owner, injector(&events));
        session.injector.ready = false;
        assert!(session.grant([1; 16]).is_err());
        session.injector.ready = true;
        session.grant([1; 16]).unwrap();
        session.apply(&[1; 16], Command::Button(0, true)).unwrap();
        session.injector.fail = true;
        assert!(session.revoke().is_err());
        assert!(session.grant([2; 16]).is_err());
        let mut other = Session::new(2, true, &owner, injector(&events));
        assert!(other.grant([3; 16]).is_err());
        assert!(!session.apply(&[1; 16], Command::Move(0, 0)).unwrap());
    }

    #[test]
    fn explicit_release_failure_invalidates_grant_and_blocks_reuse() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = Session::new(1, true, &owner, injector(&events));
        session.grant([1; 16]).unwrap();
        session.apply(&[1; 16], Command::Button(0, true)).unwrap();
        session.injector.fail = true;
        assert!(session.apply(&[1; 16], Command::ReleaseAll).is_err());
        session.injector.fail = false;
        assert!(!session.apply(&[1; 16], Command::Move(0, 0)).unwrap());
        assert!(session.grant([2; 16]).is_err());
    }

    #[test]
    fn missing_valid_input_heartbeat_releases_held_keys() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = Session::new(1, true, &owner, injector(&events));
        session.grant([1; 16]).unwrap();
        session
            .apply(&[1; 16], Command::Key("KeyA".into(), true))
            .unwrap();
        assert!(session
            .expire(Instant::now() + Duration::from_secs(4))
            .unwrap());
        assert!(!session.apply(&[1; 16], Command::Move(0, 0)).unwrap());
        assert_eq!(
            events.borrow().last(),
            Some(&Action::Press(Held::Key(0x1E, false), false))
        );
    }

    #[test]
    fn input_coordinates_keys_text_and_heartbeat_follow_the_wire_contract() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = Session::new(1, true, &owner, injector(&events));
        session.grant([1; 16]).unwrap();
        session
            .apply(&[1; 16], Command::RelativeMove(37, -23))
            .unwrap();
        assert_eq!(events.borrow().last(), Some(&Action::RelativeMove(37, -23)));
        assert!(session.apply(&[1; 16], Command::Move(65536, 0)).is_err());
        assert!(session.apply(&[1; 16], Command::Wheel(11, 0)).is_err());
        assert!(session
            .apply(&[1; 16], Command::Key("Meta".into(), true))
            .is_err());
        assert!(session
            .apply(&[1; 16], Command::Text("a".repeat(513)))
            .is_err());
        session
            .apply(&[1; 16], Command::Text("中😀".into()))
            .unwrap();
        assert!(events
            .borrow()
            .contains(&Action::Press(Held::Unicode(0xD83D), false)));
        assert!(events
            .borrow()
            .contains(&Action::Press(Held::Unicode(0xDE00), false)));
        let count = events.borrow().len();
        session.apply(&[1; 16], Command::KeepAlive).unwrap();
        assert_eq!(events.borrow().len(), count);
    }

    #[test]
    fn physical_keys_inject_matching_windows_down_and_up_events() {
        let owner = AtomicI32::new(0);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = Session::new(1, true, &owner, injector(&events));
        session.grant([1; 16]).unwrap();
        for (code, key, extended) in [
            ("F1", 0x3B, false),
            ("F2", 0x3C, false),
            ("F3", 0x3D, false),
            ("F4", 0x3E, false),
            ("F5", 0x3F, false),
            ("F6", 0x40, false),
            ("F7", 0x41, false),
            ("F8", 0x42, false),
            ("F9", 0x43, false),
            ("F10", 0x44, false),
            ("F11", 0x57, false),
            ("F12", 0x58, false),
            ("Minus", 0x0C, false),
            ("Equal", 0x0D, false),
            ("BracketLeft", 0x1A, false),
            ("BracketRight", 0x1B, false),
            ("Backslash", 0x2B, false),
            ("Semicolon", 0x27, false),
            ("Quote", 0x28, false),
            ("Backquote", 0x29, false),
            ("Comma", 0x33, false),
            ("Period", 0x34, false),
            ("Slash", 0x35, false),
            ("CapsLock", 0x3A, false),
            ("Insert", 0x52, true),
            ("NumLock", 0x45, true),
            ("ScrollLock", 0x46, false),
            ("MetaLeft", 0x5B, true),
            ("MetaRight", 0x5C, true),
            ("ShiftLeft", 0x2A, false),
            ("ShiftRight", 0x36, false),
            ("ControlLeft", 0x1D, false),
            ("ControlRight", 0x1D, true),
            ("AltLeft", 0x38, false),
            ("AltRight", 0x38, true),
            ("ContextMenu", 0x5D, true),
            ("PrintScreen", 0x37, true),
            ("Numpad0", 0x52, false),
            ("Numpad1", 0x4F, false),
            ("Numpad2", 0x50, false),
            ("Numpad3", 0x51, false),
            ("Numpad4", 0x4B, false),
            ("Numpad5", 0x4C, false),
            ("Numpad6", 0x4D, false),
            ("Numpad7", 0x47, false),
            ("Numpad8", 0x48, false),
            ("Numpad9", 0x49, false),
            ("NumpadAdd", 0x4E, false),
            ("NumpadSubtract", 0x4A, false),
            ("NumpadMultiply", 0x37, false),
            ("NumpadDivide", 0x35, true),
            ("NumpadDecimal", 0x53, false),
            ("NumpadEnter", 0x1C, true),
        ] {
            events.borrow_mut().clear();
            for down in [true, false] {
                assert!(session
                    .apply(&[1; 16], Command::Key(code.into(), down))
                    .unwrap());
            }
            assert_eq!(
                *events.borrow(),
                [
                    Action::Press(Held::Key(key, extended), true),
                    Action::Press(Held::Key(key, extended), false),
                ],
                "{code}"
            );
        }
        for code in [
            "F0",
            "F13",
            "F01",
            "Numpad10",
            "NumpadX",
            "CtrlAltDel",
            "Unknown",
        ] {
            assert!(
                session
                    .apply(&[1; 16], Command::Key(code.into(), true))
                    .is_err(),
                "{code}"
            );
        }
    }

    #[cfg(feature = "ord-secure-host")]
    #[test]
    fn windows_native_input_keeps_scan_codes_and_relative_deltas() {
        use super::runtime::native_input;
        use winapi::um::winuser::{KEYEVENTF_SCANCODE, MOUSEEVENTF_MOVE};

        let key = native_input(&Action::Press(Held::Key(0x1d, false), true));
        let relative = native_input(&Action::RelativeMove(37, -23));
        unsafe {
            let key = *key.u.ki();
            assert_eq!(key.wVk, 0);
            assert_eq!(key.wScan, 0x1d);
            assert_eq!(key.dwFlags, KEYEVENTF_SCANCODE);

            let mouse = *relative.u.mi();
            assert_eq!(mouse.dx, 37);
            assert_eq!(mouse.dy, -23);
            assert_eq!(mouse.dwFlags, MOUSEEVENTF_MOVE);
        }
    }
}
