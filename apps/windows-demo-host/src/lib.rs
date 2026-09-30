use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use bytes::BytesMut;
use protobuf::Message as _;
use remote_controller_core::{
    protos::{
        message::{
            self, login_response, permission_info, Hash, LoginResponse, Message, Misc, PeerInfo,
            PermissionInfo, SignedId,
        },
        rendezvous::IdPk,
    },
    session::approval_code,
    upstream_crypto::{Encrypt, KxTranscript},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sodiumoxide::{
    crypto::{box_, secretbox, sign},
    randombytes,
};

const FRAME_LIMIT: usize = 64 * 1024;
const IO_SLICE: Duration = Duration::from_millis(50);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_LIMIT: Duration = Duration::from_secs(30 * 60);
const COMMAND_LIMIT: usize = 32;
const EVENT_LIMIT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostState {
    Listening,
    Handshaking,
    Pending,
    Active,
}

#[derive(Clone, Debug)]
pub struct HostSnapshot {
    pub session_id: Option<u64>,
    pub state: HostState,
    pub requester_id: String,
    pub requester_name: String,
    pub pair_code: String,
    pub input_allowed: bool,
    pub x: i32,
    pub y: i32,
    pub text: String,
}

impl Default for HostSnapshot {
    fn default() -> Self {
        Self {
            session_id: None,
            state: HostState::Listening,
            requester_id: String::new(),
            requester_name: String::new(),
            pair_code: String::new(),
            input_allowed: false,
            x: 400,
            y: 225,
            text: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostEvent {
    Pending {
        session_id: u64,
        pair_code: String,
        requester_id: String,
        requester_name: String,
    },
    Approved {
        session_id: u64,
    },
    InputAllowed {
        session_id: u64,
    },
    InputRevoked {
        session_id: u64,
    },
    Rejected {
        session_id: u64,
    },
    TimedOut {
        session_id: u64,
    },
    Disconnected {
        session_id: u64,
    },
    Updated {
        session_id: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    Approve,
    Reject,
    AllowInput,
    RevokeInput,
    Disconnect,
}

struct Shared {
    snapshot: HostSnapshot,
    events: VecDeque<HostEvent>,
    commands: VecDeque<(u64, Command)>,
    next_session_id: u64,
    closing: bool,
}

impl Shared {
    fn event(&mut self, event: HostEvent) {
        if self.events.len() == EVENT_LIMIT {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

#[derive(Clone)]
pub struct HostHandle {
    shared: Arc<Mutex<Shared>>,
    stopped: Arc<AtomicBool>,
}

impl HostHandle {
    pub fn snapshot(&self) -> HostSnapshot {
        self.shared.lock().unwrap().snapshot.clone()
    }
    pub fn poll_event(&self) -> Option<HostEvent> {
        self.shared.lock().unwrap().events.pop_front()
    }
    fn command(&self, id: u64, command: Command) -> bool {
        let mut shared = self.shared.lock().unwrap();
        let valid = !shared.closing
            && shared.snapshot.session_id == Some(id)
            && match command {
                Command::Approve | Command::Reject => shared.snapshot.state == HostState::Pending,
                Command::AllowInput | Command::RevokeInput => {
                    shared.snapshot.state == HostState::Active
                }
                Command::Disconnect => matches!(
                    shared.snapshot.state,
                    HostState::Pending | HostState::Active
                ),
            };
        if valid {
            if matches!(
                command,
                Command::RevokeInput | Command::Disconnect | Command::Reject
            ) {
                shared.commands.retain(|(session, _)| *session != id);
                shared.snapshot.input_allowed = false;
            } else if shared.commands.len() >= COMMAND_LIMIT {
                return false;
            }
            if command == Command::AllowInput {
                shared.snapshot.input_allowed = true;
            }
            if matches!(command, Command::Disconnect | Command::Reject) {
                shared.closing = true;
            }
            shared.commands.push_back((id, command));
        }
        valid
    }
    pub fn approve(&self, session_id: u64) -> bool {
        self.command(session_id, Command::Approve)
    }
    pub fn reject(&self, session_id: u64) -> bool {
        self.command(session_id, Command::Reject)
    }
    pub fn allow_input(&self, session_id: u64) -> bool {
        self.command(session_id, Command::AllowInput)
    }
    pub fn revoke_input(&self, session_id: u64) -> bool {
        self.command(session_id, Command::RevokeInput)
    }
    pub fn disconnect(&self, session_id: u64) -> bool {
        self.command(session_id, Command::Disconnect)
    }
    pub fn stop(&self) {
        let mut shared = self.shared.lock().unwrap();
        shared.closing = true;
        shared.snapshot.input_allowed = false;
        self.stopped.store(true, Ordering::Release);
    }
}

pub struct DemoServer {
    handle: HostHandle,
    worker: Option<JoinHandle<()>>,
    local_addr: SocketAddr,
    public_profile_json: String,
}

impl DemoServer {
    pub fn start(addr: SocketAddr) -> io::Result<Self> {
        Self::start_with_approval_timeout(addr, HANDSHAKE_TIMEOUT)
    }
    pub fn start_with_approval_timeout(
        addr: SocketAddr,
        approval_timeout: Duration,
    ) -> io::Result<Self> {
        if addr.ip().is_unspecified() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a concrete listen IP is required",
            ));
        }
        if approval_timeout.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "approval timeout must be positive",
            ));
        }
        sodiumoxide::init().map_err(|_| io::Error::other("crypto initialization failed"))?;
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let (public, secret) = sign::gen_keypair();
        let random = randombytes::randombytes(8);
        let peer_id = format!(
            "{:09}",
            u64::from_be_bytes(
                random
                    .try_into()
                    .map_err(|_| io::Error::other("random failure"))?
            ) % 1_000_000_000
        );
        let key_b64 = STANDARD.encode(public.0);
        let fingerprint = format!("{:x}", Sha256::digest(public.0));
        let public_profile_json = json!({
            "id":peer_id, "name":"Windows demo host", "mode":"direct", "target":local_addr.to_string(),
            "server":"", "serverKey":"", "peerFingerprint":fingerprint,
            "peerId":peer_id, "peerPublicKey":key_b64,
        }).to_string();
        let handle = HostHandle {
            shared: Arc::new(Mutex::new(Shared {
                snapshot: HostSnapshot::default(),
                events: VecDeque::new(),
                commands: VecDeque::new(),
                next_session_id: 1,
                closing: false,
            })),
            stopped: Arc::new(AtomicBool::new(false)),
        };
        let worker_handle = handle.clone();
        let worker = thread::Builder::new()
            .name("demo-host-network".into())
            .spawn(move || serve(listener, worker_handle, peer_id, secret, approval_timeout))?;
        Ok(Self {
            handle,
            worker: Some(worker),
            local_addr,
            public_profile_json,
        })
    }
    pub fn handle(&self) -> HostHandle {
        self.handle.clone()
    }
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    pub fn public_profile_json(&self) -> &str {
        &self.public_profile_json
    }
    pub fn stop(&mut self) {
        self.handle.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for DemoServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve(
    listener: TcpListener,
    handle: HostHandle,
    peer_id: String,
    secret: sign::SecretKey,
    approval_timeout: Duration,
) {
    while !handle.stopped.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let id = {
                    let mut shared = handle.shared.lock().unwrap();
                    let id = shared.next_session_id;
                    shared.next_session_id = shared.next_session_id.wrapping_add(1);
                    shared.snapshot = HostSnapshot {
                        session_id: Some(id),
                        state: HostState::Handshaking,
                        ..HostSnapshot::default()
                    };
                    shared.closing = false;
                    id
                };
                let result = connection(
                    &mut stream,
                    &handle,
                    id,
                    &peer_id,
                    &secret,
                    approval_timeout,
                );
                let _ = stream.shutdown(Shutdown::Both);
                let mut shared = handle.shared.lock().unwrap();
                if shared.snapshot.session_id == Some(id) {
                    shared.snapshot = HostSnapshot::default();
                    shared.commands.retain(|(session, _)| *session != id);
                    shared.event(match result {
                        Err(ConnectionEnd::Rejected) => HostEvent::Rejected { session_id: id },
                        Err(ConnectionEnd::TimedOut) => HostEvent::TimedOut { session_id: id },
                        _ => HostEvent::Disconnected { session_id: id },
                    });
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(IO_SLICE),
            Err(_) => break,
        }
    }
}

#[derive(Debug)]
enum ConnectionEnd {
    Rejected,
    TimedOut,
    Closed,
}
type ConnResult<T> = Result<T, ConnectionEnd>;

struct Wire<'a> {
    stream: &'a mut TcpStream,
    handle: &'a HostHandle,
    incoming: Vec<u8>,
}
impl Wire<'_> {
    fn check(&self, deadline: Instant) -> ConnResult<()> {
        if self.handle.stopped.load(Ordering::Acquire) {
            return Err(ConnectionEnd::Closed);
        }
        if Instant::now() >= deadline {
            return Err(ConnectionEnd::TimedOut);
        }
        Ok(())
    }
    fn receive(&mut self, deadline: Instant) -> ConnResult<Vec<u8>> {
        loop {
            if let Some(&first) = self.incoming.first() {
                let count = (first & 3) as usize + 1;
                if self.incoming.len() >= count {
                    let mut head = [0u8; 4];
                    head[..count].copy_from_slice(&self.incoming[..count]);
                    let len = (u32::from_le_bytes(head) >> 2) as usize;
                    if len > FRAME_LIMIT {
                        return Err(ConnectionEnd::Closed);
                    }
                    if self.incoming.len() >= count + len {
                        let data = self.incoming[count..count + len].to_vec();
                        self.incoming.drain(..count + len);
                        return Ok(data);
                    }
                }
            }
            self.check(deadline)?;
            self.stream
                .set_read_timeout(Some(IO_SLICE))
                .map_err(|_| ConnectionEnd::Closed)?;
            let mut chunk = [0u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(ConnectionEnd::Closed),
                Ok(n) => self.incoming.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err(ConnectionEnd::Closed),
            }
        }
    }
    fn send(&mut self, data: &[u8], deadline: Instant) -> ConnResult<()> {
        let deadline = deadline.min(Instant::now() + Duration::from_secs(2));
        if data.len() > FRAME_LIMIT {
            return Err(ConnectionEnd::Closed);
        }
        let mut frame = Vec::with_capacity(data.len() + 4);
        let count = if data.len() <= 0x3f {
            1
        } else if data.len() <= 0x3fff {
            2
        } else {
            3
        };
        let header = ((data.len() as u32) << 2 | (count - 1)).to_le_bytes();
        frame.extend_from_slice(&header[..count as usize]);
        frame.extend_from_slice(data);
        let mut rest = frame.as_slice();
        while !rest.is_empty() {
            self.check(deadline)?;
            if self.handle.shared.lock().unwrap().closing {
                return Err(ConnectionEnd::Closed);
            }
            self.stream
                .set_write_timeout(Some(IO_SLICE))
                .map_err(|_| ConnectionEnd::Closed)?;
            match self.stream.write(rest) {
                Ok(0) => return Err(ConnectionEnd::Closed),
                Ok(n) => rest = &rest[n..],
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err(ConnectionEnd::Closed),
            }
        }
        Ok(())
    }
    fn send_plain(&mut self, message: &Message, deadline: Instant) -> ConnResult<()> {
        self.send(
            &message
                .write_to_bytes()
                .map_err(|_| ConnectionEnd::Closed)?,
            deadline,
        )
    }
    fn receive_plain(&mut self, deadline: Instant) -> ConnResult<Message> {
        Message::parse_from_bytes(&self.receive(deadline)?).map_err(|_| ConnectionEnd::Closed)
    }
    fn send_encrypted(
        &mut self,
        message: &Message,
        cipher: &mut Encrypt,
        deadline: Instant,
    ) -> ConnResult<()> {
        if cipher.1 == u64::MAX {
            return Err(ConnectionEnd::Closed);
        }
        let plain = message
            .write_to_bytes()
            .map_err(|_| ConnectionEnd::Closed)?;
        self.send(&cipher.enc(&plain), deadline)
    }
    fn receive_encrypted(
        &mut self,
        cipher: &mut Encrypt,
        deadline: Instant,
    ) -> ConnResult<Message> {
        let data = self.receive(deadline)?;
        if data.len() < secretbox::MACBYTES || cipher.2 == u64::MAX {
            return Err(ConnectionEnd::Closed);
        }
        let mut data = BytesMut::from(data.as_slice());
        cipher.dec(&mut data).map_err(|_| ConnectionEnd::Closed)?;
        Message::parse_from_bytes(&data).map_err(|_| ConnectionEnd::Closed)
    }
}

fn connection(
    stream: &mut TcpStream,
    handle: &HostHandle,
    id: u64,
    peer_id: &str,
    secret: &sign::SecretKey,
    approval_timeout: Duration,
) -> ConnResult<()> {
    let started = Instant::now();
    let deadline = started + HANDSHAKE_TIMEOUT;
    let mut wire = Wire {
        stream,
        handle,
        incoming: Vec::new(),
    };
    let (our_pk, our_sk) = box_::gen_keypair();
    let identity = IdPk {
        id: peer_id.to_owned(),
        pk: our_pk.0.to_vec(),
        kx_version: 1,
        ..Default::default()
    };
    let mut signed = Message::new();
    signed.set_signed_id(SignedId {
        id: sign::sign(
            &identity
                .write_to_bytes()
                .map_err(|_| ConnectionEnd::Closed)?,
            secret,
        ),
        ..Default::default()
    });
    wire.send_plain(&signed, deadline)?;
    let public = match wire.receive_plain(deadline)?.union {
        Some(message::message::Union::PublicKey(key)) => key,
        _ => return Err(ConnectionEnd::Closed),
    };
    if public.kx_version != 1
        || public.asymmetric_value.len() != 32
        || public.symmetric_value.len() != 48
    {
        return Err(ConnectionEnd::Closed);
    }
    let key = Encrypt::decode(&public.symmetric_value, &public.asymmetric_value, &our_sk)
        .map_err(|_| ConnectionEnd::Closed)?;
    let mut cipher = Encrypt::new_split(
        key,
        false,
        &KxTranscript {
            initiator_pk: &public.asymmetric_value,
            responder_pk: &our_pk.0,
            advertised: 1,
            picked: 1,
        },
    )
    .map_err(|_| ConnectionEnd::Closed)?;
    let challenge = STANDARD.encode(randombytes::randombytes(24));
    let pair_code = approval_code(&challenge);
    let mut hash = Message::new();
    hash.set_hash(Hash {
        challenge,
        ..Default::default()
    });
    wire.send_encrypted(&hash, &mut cipher, deadline)?;
    let login = match wire.receive_encrypted(&mut cipher, deadline)?.union {
        Some(message::message::Union::LoginRequest(login)) => login,
        _ => return Err(ConnectionEnd::Closed),
    };
    if login.username != peer_id
        || !login.password.is_empty()
        || login.union.is_some()
        || login.my_id.len() > 128
        || login.my_name.len() > 128
    {
        return Err(ConnectionEnd::Rejected);
    }
    let pending_deadline = Instant::now() + approval_timeout;
    let requester_id = display_identity(&login.my_id);
    let requester_name = display_identity(&login.my_name);
    {
        let mut shared = handle.shared.lock().unwrap();
        if shared.snapshot.session_id != Some(id) || shared.closing {
            return Err(ConnectionEnd::Rejected);
        }
        shared.snapshot.state = HostState::Pending;
        shared.snapshot.requester_id = requester_id.clone();
        shared.snapshot.requester_name = requester_name.clone();
        shared.snapshot.pair_code = pair_code.clone();
        shared.event(HostEvent::Pending {
            session_id: id,
            pair_code,
            requester_id,
            requester_name,
        });
    }
    loop {
        wire.check(pending_deadline)?;
        let command = {
            let mut shared = handle.shared.lock().unwrap();
            shared.commands.pop_front()
        };
        match command {
            Some((session, Command::Approve)) if session == id => break,
            Some((session, Command::Reject | Command::Disconnect)) if session == id => {
                return Err(ConnectionEnd::Rejected)
            }
            _ => thread::sleep(IO_SLICE),
        }
    }
    if handle.shared.lock().unwrap().closing {
        return Err(ConnectionEnd::Rejected);
    }
    let mut response = Message::new();
    response.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo {
            username: "Demo".into(),
            hostname: "Windows demo".into(),
            platform: "Windows".into(),
            platform_additions:
                r#"{"ord_demo":1,"input_scope":"demo_window","width":800,"height":450}"#.into(),
            ..Default::default()
        })),
        ..Default::default()
    });
    wire.send_encrypted(
        &response,
        &mut cipher,
        deadline.max(Instant::now() + IO_SLICE),
    )?;
    for permission in [
        permission_info::Permission::Keyboard,
        permission_info::Permission::Clipboard,
        permission_info::Permission::Audio,
        permission_info::Permission::File,
        permission_info::Permission::Restart,
        permission_info::Permission::Recording,
        permission_info::Permission::BlockInput,
        permission_info::Permission::PrivacyMode,
    ] {
        send_permission(
            &mut wire,
            &mut cipher,
            permission,
            false,
            Instant::now() + Duration::from_secs(2),
        )?;
    }
    {
        let mut shared = handle.shared.lock().unwrap();
        if shared.snapshot.session_id != Some(id) || shared.closing {
            return Err(ConnectionEnd::Closed);
        }
        shared.snapshot.state = HostState::Active;
        shared.event(HostEvent::Approved { session_id: id });
    }
    let active_deadline = Instant::now() + SESSION_LIMIT;
    loop {
        wire.check(active_deadline)?;
        loop {
            let next_command = { handle.shared.lock().unwrap().commands.pop_front() };
            let Some((session, command)) = next_command else {
                break;
            };
            if session != id {
                continue;
            }
            match command {
                Command::AllowInput | Command::RevokeInput => {
                    let enabled = {
                        let mut shared = handle.shared.lock().unwrap();
                        if shared.snapshot.session_id != Some(id)
                            || shared.snapshot.state != HostState::Active
                            || shared.closing
                        {
                            return Err(ConnectionEnd::Closed);
                        }
                        let enabled = shared.snapshot.input_allowed;
                        shared.event(if enabled {
                            HostEvent::InputAllowed { session_id: id }
                        } else {
                            HostEvent::InputRevoked { session_id: id }
                        });
                        enabled
                    };
                    send_permission(
                        &mut wire,
                        &mut cipher,
                        permission_info::Permission::Keyboard,
                        enabled,
                        active_deadline,
                    )?;
                }
                Command::Disconnect => return Err(ConnectionEnd::Closed),
                _ => {}
            }
        }
        // A short deadline lets GUI commands and stop take effect without a second socket owner.
        let read_deadline = Instant::now() + IO_SLICE;
        let message = match wire.receive_encrypted(&mut cipher, read_deadline) {
            Ok(message) => message,
            Err(ConnectionEnd::TimedOut) => continue,
            Err(e) => return Err(e),
        };
        let changed = {
            let mut shared = handle.shared.lock().unwrap();
            if shared.snapshot.session_id != Some(id)
                || shared.snapshot.state != HostState::Active
                || shared.closing
            {
                return Err(ConnectionEnd::Closed);
            }
            let changed =
                apply_input(&mut shared.snapshot, message).map_err(|_| ConnectionEnd::Closed)?;
            if changed {
                shared.event(HostEvent::Updated { session_id: id });
            }
            changed
        };
        if changed {
            let snapshot = handle.snapshot();
            let mut misc = Misc::new();
            misc.set_chat_message(message::ChatMessage { text: json!({"ord_demo_status":1,"x":snapshot.x,"y":snapshot.y,"textLength":snapshot.text.chars().count()}).to_string(), ..Default::default() });
            let mut status = Message::new();
            status.set_misc(misc);
            wire.send_encrypted(&status, &mut cipher, active_deadline)?;
        }
    }
}

fn display_identity(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
        .take(64)
        .collect()
}

fn apply_input(snapshot: &mut HostSnapshot, message: Message) -> Result<bool, ()> {
    match message.union {
        Some(message::message::Union::MouseEvent(mouse))
            if mouse.mask == 0
                && mouse.modifiers.is_empty()
                && (0..800).contains(&mouse.x)
                && (0..450).contains(&mouse.y) =>
        {
            if snapshot.state != HostState::Active || !snapshot.input_allowed {
                return Ok(false);
            }
            snapshot.x = mouse.x;
            snapshot.y = mouse.y;
            Ok(true)
        }
        Some(message::message::Union::KeyEvent(key))
            if !key.down
                && !key.press
                && key.modifiers.is_empty()
                && key.mode == Default::default() =>
        {
            if let Some(message::key_event::Union::Seq(seq)) = key.union {
                if !seq.is_empty() && seq.len() <= 512 {
                    if snapshot.state != HostState::Active || !snapshot.input_allowed {
                        return Ok(false);
                    }
                    if snapshot.text.len() + seq.len() <= 16 * 1024 {
                        snapshot.text.push_str(&seq);
                        return Ok(true);
                    }
                    return Ok(false);
                }
            }
            Err(())
        }
        _ => Err(()),
    }
}

fn send_permission(
    wire: &mut Wire<'_>,
    cipher: &mut Encrypt,
    permission: permission_info::Permission,
    enabled: bool,
    deadline: Instant,
) -> ConnResult<()> {
    let mut misc = Misc::new();
    misc.set_permission_info(PermissionInfo {
        permission: permission.into(),
        enabled,
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_misc(misc);
    wire.send_encrypted(&message, cipher, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_input_gate_rejects_raw_messages_without_current_permission() {
        let mut snapshot = HostSnapshot {
            state: HostState::Active,
            session_id: Some(1),
            ..HostSnapshot::default()
        };
        let mut pointer = Message::new();
        pointer.set_mouse_event(message::MouseEvent {
            x: 77,
            y: 88,
            ..Default::default()
        });
        assert_eq!(apply_input(&mut snapshot, pointer.clone()), Ok(false));
        assert_eq!((snapshot.x, snapshot.y), (400, 225));
        snapshot.input_allowed = true;
        assert_eq!(apply_input(&mut snapshot, pointer.clone()), Ok(true));
        snapshot.input_allowed = false;
        assert_eq!(apply_input(&mut snapshot, pointer), Ok(false));
        assert_eq!((snapshot.x, snapshot.y), (77, 88));
        let mut key = message::KeyEvent::new();
        key.set_seq("forbidden".into());
        let mut text = Message::new();
        text.set_key_event(key);
        assert_eq!(apply_input(&mut snapshot, text), Ok(false));
        assert!(snapshot.text.is_empty());
        let mut unsupported = Message::new();
        unsupported.set_clipboard(message::Clipboard::new());
        assert_eq!(apply_input(&mut snapshot, unsupported), Err(()));
    }

    #[test]
    fn revoke_overrides_a_grant_already_taken_by_network_thread() {
        let handle = HostHandle {
            shared: Arc::new(Mutex::new(Shared {
                snapshot: HostSnapshot {
                    session_id: Some(7),
                    state: HostState::Active,
                    ..HostSnapshot::default()
                },
                events: VecDeque::new(),
                commands: VecDeque::new(),
                next_session_id: 8,
                closing: false,
            })),
            stopped: Arc::new(AtomicBool::new(false)),
        };
        assert!(handle.allow_input(7));
        let taken = { handle.shared.lock().unwrap().commands.pop_front() };
        assert!(matches!(taken, Some((7, Command::AllowInput))));
        assert!(handle.revoke_input(7));
        assert!(!handle.snapshot().input_allowed);
        assert!(!handle
            .shared
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|(_, command)| *command == Command::AllowInput));
    }

    #[test]
    fn untrusted_requester_label_cannot_add_lines_or_direction_controls() {
        assert_eq!(
            display_identity("Alice\r\npair code: 000000\u{202e}"),
            "Alicepair code 000000"
        );
    }

    #[test]
    fn local_disconnect_interrupts_a_send() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let _peer = listener.accept().unwrap();
        let handle = HostHandle {
            shared: Arc::new(Mutex::new(Shared {
                snapshot: HostSnapshot {
                    session_id: Some(9),
                    state: HostState::Active,
                    input_allowed: true,
                    ..HostSnapshot::default()
                },
                events: VecDeque::new(),
                commands: VecDeque::new(),
                next_session_id: 10,
                closing: false,
            })),
            stopped: Arc::new(AtomicBool::new(false)),
        };
        assert!(handle.disconnect(9));
        let mut wire = Wire {
            stream: &mut stream,
            handle: &handle,
            incoming: Vec::new(),
        };
        assert!(matches!(
            wire.send(b"status", Instant::now() + SESSION_LIMIT),
            Err(ConnectionEnd::Closed)
        ));
        assert!(!handle.snapshot().input_allowed);
    }
}
