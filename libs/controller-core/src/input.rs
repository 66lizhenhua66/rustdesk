use std::collections::VecDeque;

use serde::Deserialize;

use crate::protos::message::{
    self, Message, OrdInputButton, OrdInputEvent, OrdInputKeepAlive, OrdInputKey, OrdInputMove,
    OrdInputRelativeMove, OrdInputReleaseAll, OrdInputRequest, OrdInputState, OrdInputText,
    OrdInputWheel,
};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Button {
    Left,
    Right,
    Middle,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum InputCommand {
    Move { x: u32, y: u32 },
    MoveRelative { dx: i32, dy: i32 },
    Button { button: Button, down: bool },
    Wheel { dx: i32, dy: i32 },
    Key { code: String, down: bool },
    Text { text: String },
    ReleaseAll,
}

pub(crate) fn parse_command(raw: &str) -> Result<InputCommand, ()> {
    if raw.len() > 1024 {
        return Err(());
    }
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|_| ())?;
    let fields = value.as_object().ok_or(())?.len();
    let command: InputCommand = serde_json::from_value(value).map_err(|_| ())?;
    let expected_fields = match &command {
        InputCommand::Move { .. }
        | InputCommand::MoveRelative { .. }
        | InputCommand::Wheel { .. } => 3,
        InputCommand::Button { .. } | InputCommand::Key { .. } => 3,
        InputCommand::Text { .. } => 2,
        InputCommand::ReleaseAll => 1,
    };
    if fields != expected_fields {
        return Err(());
    }
    let valid = match &command {
        InputCommand::Move { x, y } => *x <= 65_535 && *y <= 65_535,
        InputCommand::MoveRelative { dx, dy } => {
            (-65_535..=65_535).contains(dx) && (-65_535..=65_535).contains(dy)
        }
        InputCommand::Button { .. } => true,
        InputCommand::Wheel { dx, dy } => (-10..=10).contains(dx) && (-10..=10).contains(dy),
        InputCommand::Key { code, .. } => valid_key_code(code),
        InputCommand::Text { text } => !text.is_empty() && text.len() <= 512,
        InputCommand::ReleaseAll => true,
    };
    if valid {
        Ok(command)
    } else {
        Err(())
    }
}

fn valid_key_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    (bytes.len() == 4 && bytes.starts_with(b"Key") && bytes[3].is_ascii_uppercase())
        || (bytes.len() == 6 && bytes.starts_with(b"Digit") && bytes[5].is_ascii_digit())
        || (bytes.len() == 7 && bytes.starts_with(b"Numpad") && bytes[6].is_ascii_digit())
        || matches!(
            code,
            "Enter"
                | "Escape"
                | "Tab"
                | "Space"
                | "Backspace"
                | "Delete"
                | "ArrowLeft"
                | "ArrowRight"
                | "ArrowUp"
                | "ArrowDown"
                | "Home"
                | "End"
                | "PageUp"
                | "PageDown"
                | "Shift"
                | "Control"
                | "Alt"
                | "F1"
                | "F2"
                | "F3"
                | "F4"
                | "F5"
                | "F6"
                | "F7"
                | "F8"
                | "F9"
                | "F10"
                | "F11"
                | "F12"
                | "Minus"
                | "Equal"
                | "BracketLeft"
                | "BracketRight"
                | "Backslash"
                | "Semicolon"
                | "Quote"
                | "Backquote"
                | "Comma"
                | "Period"
                | "Slash"
                | "CapsLock"
                | "Insert"
                | "NumLock"
                | "ScrollLock"
                | "MetaLeft"
                | "MetaRight"
                | "NumpadAdd"
                | "NumpadSubtract"
                | "NumpadMultiply"
                | "NumpadDivide"
                | "NumpadDecimal"
                | "NumpadEnter"
        )
}

impl InputCommand {
    pub(crate) fn is_release(&self) -> bool {
        matches!(
            self,
            Self::ReleaseAll | Self::Button { down: false, .. } | Self::Key { down: false, .. }
        )
    }

    pub(crate) fn is_move(&self) -> bool {
        matches!(self, Self::Move { .. })
    }

    pub(crate) fn into_message(self, token: [u8; 16]) -> Message {
        let mut event = OrdInputEvent::new();
        event.version = 1;
        event.grant_token = token.to_vec();
        match self {
            Self::Move { x, y } => event.set_move(OrdInputMove {
                x,
                y,
                ..Default::default()
            }),
            Self::MoveRelative { dx, dy } => event.set_move_relative(OrdInputRelativeMove {
                dx,
                dy,
                ..Default::default()
            }),
            Self::Button { button, down } => {
                let button = match button {
                    Button::Left => message::ord_input_button::Button::Left,
                    Button::Right => message::ord_input_button::Button::Right,
                    Button::Middle => message::ord_input_button::Button::Middle,
                };
                event.set_button(OrdInputButton {
                    button: button.into(),
                    down,
                    ..Default::default()
                });
            }
            Self::Wheel { dx, dy } => event.set_wheel(OrdInputWheel {
                dx,
                dy,
                ..Default::default()
            }),
            Self::Key { code, down } => event.set_key(OrdInputKey {
                code,
                down,
                ..Default::default()
            }),
            Self::Text { text } => event.set_text(OrdInputText {
                text,
                ..Default::default()
            }),
            Self::ReleaseAll => event.set_release_all(OrdInputReleaseAll::new()),
        }
        let mut message = Message::new();
        message.set_ord_input_event(event);
        message
    }
}

#[derive(Default)]
pub(crate) struct ControlInputState {
    supported: bool,
    token: Option<[u8; 16]>,
    pending: VecDeque<InputCommand>,
    requests: VecDeque<OrdInputRequest>,
    latest_request_id: u64,
    desired_enabled: bool,
}

impl ControlInputState {
    pub(crate) fn clear(&mut self) {
        self.supported = false;
        self.token = None;
        self.pending.clear();
        self.requests.clear();
        self.desired_enabled = false;
    }

    pub(crate) fn apply_state(
        &mut self,
        state: &OrdInputState,
    ) -> Result<Option<(bool, bool)>, ()> {
        if state.version != 1
            || state
                .special_fields
                .unknown_fields()
                .iter()
                .next()
                .is_some()
            || state.enabled && !state.supported
            || state.enabled && state.grant_token.len() != 16
            || !state.enabled && !state.grant_token.is_empty()
            || state.request_id > self.latest_request_id
            || state.request_id == 0 && state.enabled
        {
            self.clear();
            return Err(());
        }
        if state.request_id != 0 && state.request_id < self.latest_request_id {
            return Ok(None);
        }
        if state.enabled && !self.desired_enabled {
            self.clear();
            return Err(());
        }
        let token = if state.enabled {
            let mut token = [0; 16];
            token.copy_from_slice(&state.grant_token);
            Some(token)
        } else {
            None
        };
        if self.token != token {
            self.pending.clear();
        }
        self.supported = state.supported;
        self.token = token;
        Ok(Some((self.supported, self.token.is_some())))
    }

    pub(crate) fn request_enabled(&mut self, enabled: bool) -> Result<u64, i32> {
        if enabled && !self.supported {
            return Err(2);
        }
        if !enabled {
            self.token = None;
            self.pending.clear();
            self.requests.clear();
        } else if self.requests.len() >= 8 {
            return Err(4);
        }
        let Some(request_id) = self.latest_request_id.checked_add(1) else {
            return Err(4);
        };
        self.requests.try_reserve(1).map_err(|_| 4)?;
        self.latest_request_id = request_id;
        self.desired_enabled = enabled;
        self.requests.push_back(OrdInputRequest {
            version: 1,
            enabled,
            scope: "windows_primary".to_owned(),
            request_id,
            ..Default::default()
        });
        Ok(request_id)
    }

    pub(crate) fn pop_request(&mut self) -> Option<OrdInputRequest> {
        self.requests.pop_front()
    }

    pub(crate) fn supported(&self) -> bool {
        self.supported
    }

    pub(crate) fn allowed(&self) -> bool {
        self.token.is_some()
    }

    pub(crate) fn queue(&mut self, command: InputCommand) -> i32 {
        if self.token.is_none() {
            return 2;
        }
        if matches!(command, InputCommand::ReleaseAll) {
            self.pending.clear();
            self.pending.push_front(InputCommand::ReleaseAll);
            return 0;
        }
        if command.is_move() && matches!(self.pending.back(), Some(InputCommand::Move { .. })) {
            self.pending.pop_back();
        }
        if self.pending.len() >= 64 {
            if command.is_release() {
                self.pending.clear();
                self.pending.push_front(InputCommand::ReleaseAll);
                return 0;
            }
            return 4;
        }
        self.pending.push_back(command);
        0
    }

    pub(crate) fn pop(&mut self) -> Option<(InputCommand, [u8; 16])> {
        let token = self.token?;
        Some((self.pending.pop_front()?, token))
    }

    pub(crate) fn matches_token(&self, token: [u8; 16]) -> bool {
        self.token == Some(token)
    }

    pub(crate) fn keep_alive_message(&self) -> Option<Message> {
        let token = self.token?;
        let mut event = OrdInputEvent::new();
        event.version = 1;
        event.grant_token = token.to_vec();
        event.set_keep_alive(OrdInputKeepAlive::new());
        let mut message = Message::new();
        message.set_ord_input_event(event);
        Some(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protos::message::OrdInputState;

    fn active_state(token: u8) -> (ControlInputState, u64) {
        let mut state = ControlInputState::default();
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                request_id: 0,
                ..Default::default()
            })
            .unwrap();
        let request_id = state.request_enabled(true).unwrap();
        state.pop_request();
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![token; 16],
                request_id,
                ..Default::default()
            })
            .unwrap();
        (state, request_id)
    }

    #[test]
    fn bounded_control_commands_parse_with_exact_shapes() {
        assert!(matches!(
            parse_command(r#"{"kind":"move","x":0,"y":65535}"#),
            Ok(InputCommand::Move { x: 0, y: 65535 })
        ));
        assert!(matches!(
            parse_command(r#"{"kind":"move_relative","dx":-65535,"dy":65535}"#),
            Ok(InputCommand::MoveRelative {
                dx: -65535,
                dy: 65535
            })
        ));
        assert!(matches!(
            parse_command(r#"{"kind":"wheel","dx":-10,"dy":10}"#),
            Ok(InputCommand::Wheel { dx: -10, dy: 10 })
        ));
        assert!(matches!(
            parse_command(r#"{"kind":"key","code":"KeyA","down":true}"#),
            Ok(InputCommand::Key { .. })
        ));
        assert!(matches!(
            parse_command(r#"{"kind":"key","code":"Space","down":true}"#),
            Ok(InputCommand::Key { .. })
        ));
        assert!(matches!(
            parse_command(r#"{"kind":"release_all"}"#),
            Ok(InputCommand::ReleaseAll)
        ));
    }

    #[test]
    fn malformed_or_unbounded_control_commands_are_rejected() {
        for value in [
            r#"{"kind":"move","x":65536,"y":0}"#,
            r#"{"kind":"move_relative","dx":65536,"dy":0}"#,
            r#"{"kind":"move","x":1,"y":2,"authorized":true}"#,
            r#"{"kind":"wheel","dx":11,"dy":0}"#,
            r#"{"kind":"key","code":"CtrlAltDel","down":true}"#,
            r#"{"kind":"button","button":"side","down":true}"#,
            r#"{"kind":"text","text":""}"#,
            r#"{"kind":"release_all","token":"forged"}"#,
        ] {
            assert!(parse_command(value).is_err(), "{value}");
        }
    }

    #[test]
    fn physical_keyboard_commands_preserve_codes_and_release_edges() {
        for code in "F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12
            Minus Equal BracketLeft BracketRight Backslash Semicolon Quote Backquote Comma Period Slash
            CapsLock Insert NumLock ScrollLock MetaLeft MetaRight
            Numpad0 Numpad1 Numpad2 Numpad3 Numpad4 Numpad5 Numpad6 Numpad7 Numpad8 Numpad9
            NumpadAdd NumpadSubtract NumpadMultiply NumpadDivide NumpadDecimal NumpadEnter"
            .split_whitespace()
        {
            for down in [true, false] {
                let raw = serde_json::json!({"kind": "key", "code": code, "down": down}).to_string();
                let command = parse_command(&raw).unwrap_or_else(|_| panic!("Rejected {code}"));
                let message = command.into_message([7; 16]);
                let Some(message::message::Union::OrdInputEvent(event)) = message.union else {
                    panic!("Missing input event for {code}")
                };
                assert_eq!(event.grant_token, [7; 16]);
                let Some(message::ord_input_event::Command::Key(key)) = event.command else {
                    panic!("Missing key event for {code}")
                };
                assert_eq!((key.code.as_str(), key.down), (code, down));
            }
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
            let raw = serde_json::json!({"kind": "key", "code": code, "down": true}).to_string();
            assert!(parse_command(&raw).is_err(), "Accepted {code}");
        }
    }

    #[test]
    fn revoke_discards_pending_input_and_new_grant_cannot_replay_it() {
        let mut state = ControlInputState::default();
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"move","x":1,"y":2}"#).unwrap()),
            2
        );
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                request_id: 0,
                ..Default::default()
            })
            .unwrap();
        let first_on = state.request_enabled(true).unwrap();
        state.pop_request();
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![1; 16],
                request_id: first_on,
                ..Default::default()
            }),
            Ok(Some((true, true)))
        );
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"move","x":1,"y":2}"#).unwrap()),
            0
        );
        let off = state.request_enabled(false).unwrap();
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                request_id: off,
                ..Default::default()
            }),
            Ok(Some((true, false)))
        );
        let second_on = state.request_enabled(true).unwrap();
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![2; 16],
                request_id: second_on,
                ..Default::default()
            }),
            Ok(Some((true, true)))
        );
        assert!(state.pop().is_none());
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"key","code":"KeyA","down":true}"#).unwrap()),
            0
        );
        let (_, token) = state.pop().unwrap();
        assert_eq!(token, [2; 16]);
    }

    #[test]
    fn repeated_grant_with_same_token_keeps_pending_key_release() {
        let (mut state, request_id) = active_state(5);
        let grant = OrdInputState {
            version: 1,
            supported: true,
            enabled: true,
            grant_token: vec![5; 16],
            request_id,
            ..Default::default()
        };
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"key","code":"KeyA","down":false}"#).unwrap()),
            0
        );
        state.apply_state(&grant).unwrap();
        assert!(
            matches!(state.pop(), Some((InputCommand::Key { down: false, .. }, token)) if token == [5; 16])
        );
    }

    #[test]
    fn release_all_preempts_a_full_queue() {
        let (mut state, _) = active_state(3);
        for _ in 0..64 {
            assert_eq!(
                state.queue(parse_command(r#"{"kind":"text","text":"a"}"#).unwrap()),
                0
            );
        }
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"key","code":"KeyA","down":true}"#).unwrap()),
            4
        );
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"release_all"}"#).unwrap()),
            0
        );
        assert!(matches!(state.pop(), Some((InputCommand::ReleaseAll, token)) if token == [3; 16]));
        assert!(state.pop().is_none());
        for _ in 0..64 {
            assert_eq!(
                state.queue(parse_command(r#"{"kind":"text","text":"a"}"#).unwrap()),
                0
            );
        }
        assert_eq!(
            state
                .queue(parse_command(r#"{"kind":"button","button":"left","down":false}"#).unwrap()),
            0
        );
        assert!(matches!(state.pop(), Some((InputCommand::ReleaseAll, _))));
        assert!(state.pop().is_none());
    }

    #[test]
    fn invalid_or_unknown_state_cannot_enable_input() {
        let mut state = ControlInputState::default();
        assert!(state
            .apply_state(&OrdInputState {
                version: 2,
                supported: true,
                enabled: true,
                grant_token: vec![1; 16],
                ..Default::default()
            })
            .is_err());
        assert!(state
            .apply_state(&OrdInputState {
                version: 1,
                supported: false,
                enabled: true,
                grant_token: vec![1; 16],
                ..Default::default()
            })
            .is_err());
        assert!(state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: vec![1; 16],
                ..Default::default()
            })
            .is_err());
        assert!(state.pop().is_none());
    }

    #[test]
    fn heartbeat_uses_current_grant_and_stops_after_revoke() {
        let mut state = ControlInputState::default();
        assert!(state.keep_alive_message().is_none());
        let (active, _) = active_state(4);
        state = active;
        let heartbeat = state.keep_alive_message().unwrap();
        let Some(message::message::Union::OrdInputEvent(event)) = heartbeat.union else {
            panic!("expected input heartbeat")
        };
        assert_eq!(event.grant_token, vec![4; 16]);
        assert!(matches!(
            event.command,
            Some(message::ord_input_event::Command::KeepAlive(_))
        ));
        let off = state.request_enabled(false).unwrap();
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                request_id: off,
                ..Default::default()
            })
            .unwrap();
        assert!(state.keep_alive_message().is_none());
    }

    #[test]
    fn rapid_off_then_on_sends_revoke_first_and_ignores_old_grant() {
        let mut state = ControlInputState::default();
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                request_id: 0,
                ..Default::default()
            })
            .unwrap();
        let first_on = state.request_enabled(true).unwrap();
        let first_request = state.pop_request().unwrap();
        assert_eq!(
            (first_request.request_id, first_request.enabled),
            (first_on, true)
        );
        let off = state.request_enabled(false).unwrap();
        let second_on = state.request_enabled(true).unwrap();
        assert!(first_on < off && off < second_on);
        let revoke = state.pop_request().unwrap();
        let regrant = state.pop_request().unwrap();
        assert_eq!((revoke.request_id, revoke.enabled), (off, false));
        assert_eq!((regrant.request_id, regrant.enabled), (second_on, true));
        assert!(state.pop_request().is_none());
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![1; 16],
                request_id: first_on,
                ..Default::default()
            }),
            Ok(None)
        );
        assert!(!state.allowed());
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                request_id: off,
                ..Default::default()
            }),
            Ok(None)
        );
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![2; 16],
                request_id: second_on,
                ..Default::default()
            }),
            Ok(Some((true, true)))
        );
        assert!(state.allowed());
    }

    #[test]
    fn off_clears_input_immediately_and_unrequested_grants_fail_closed() {
        let mut state = ControlInputState::default();
        assert!(state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![1; 16],
                request_id: 0,
                ..Default::default()
            })
            .is_err());
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                request_id: 0,
                ..Default::default()
            })
            .unwrap();
        let on = state.request_enabled(true).unwrap();
        state.pop_request();
        state
            .apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![3; 16],
                request_id: on,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            state.queue(parse_command(r#"{"kind":"key","code":"KeyA","down":true}"#).unwrap()),
            0
        );
        let off = state.request_enabled(false).unwrap();
        assert!(!state.allowed());
        assert!(state.pop().is_none());
        assert_eq!(state.pop_request().unwrap().request_id, off);
        assert_eq!(
            state.apply_state(&OrdInputState {
                version: 1,
                supported: true,
                enabled: true,
                grant_token: vec![3; 16],
                request_id: on,
                ..Default::default()
            }),
            Ok(None)
        );
        assert!(!state.allowed());
    }
}
