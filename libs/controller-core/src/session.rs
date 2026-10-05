use std::{
    collections::VecDeque,
    ffi::{c_char, c_void, CStr, CString},
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::{Bytes, BytesMut};
use protobuf::{Message as _, MessageField};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sodiumoxide::crypto::{secretbox, sign};
use tokio_util::codec::{Decoder, Encoder};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    bytes_codec::BytesCodec,
    input::{parse_command, ControlInputState},
    protos::message::{
        self, login_response, option_message, permission_info, Hash, KeyEvent, LoginRequest,
        Message, Misc, MouseEvent, OptionMessage, PublicKey, SupportedDecoding,
    },
    upstream_crypto::{self, Encrypt, KxTranscript, KX_VERSION_LATEST},
};

const FRAME_LIMIT: usize = 64 * 1024;
const VIDEO_FRAME_LIMIT: usize = 8 * 1024 * 1024;
const VIDEO_PART_LIMIT: usize = 2 * 1024 * 1024;
const SLICE: Duration = Duration::from_millis(100);
const DEMO_LIMIT: usize = 64;
const DEMO_LIFETIME: Duration = Duration::from_secs(30 * 60);

enum DemoInput {
    Pointer(i32, i32),
    Text(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Request {
    endpoint: Option<String>,
    mode: Option<String>,
    server: Option<String>,
    server_key: Option<String>,
    relay_server: Option<String>,
    peer_id: String,
    peer_public_key: String,
    peer_fingerprint: Option<String>,
    minimum_kx_version: Option<u32>,
    expected_peer: Option<PersistentPeer>,
}

pub struct ControllerSession {
    endpoint: Option<SocketAddr>,
    pub(crate) rendezvous: Option<crate::rendezvous::Config>,
    pub(crate) peer_id: String,
    pub(crate) peer_key: sign::PublicKey,
    password: Mutex<Option<Zeroizing<String>>>,
    access: Mutex<Option<crate::access::Access>>,
    timeout: Duration,
    started: AtomicBool,
    cancelled: AtomicBool,
    pub(crate) socket: Mutex<Option<TcpStream>>,
    demo: bool,
    expected_peer: PersistentPeer,
    connected: AtomicBool,
    authorized: AtomicBool,
    pending: Mutex<VecDeque<DemoInput>>,
    control_input: Mutex<ControlInputState>,
}

pub type ControllerSessionCallback = unsafe extern "C" fn(*const c_char, *mut c_void);
pub type ControllerVideoCallback =
    unsafe extern "C" fn(*const u8, u32, u32, u32, i64, u8, *mut c_void);

#[no_mangle]
pub extern "C" fn controller_session_create(
    request_json: *const c_char,
    password: *const c_char,
    timeout_ms: u32,
) -> *mut ControllerSession {
    create(request_json, password, timeout_ms, false)
}

#[no_mangle]
pub extern "C" fn controller_connection_create(
    request_json: *const c_char,
    password: *const c_char,
    handshake_ms: u32,
) -> *mut ControllerSession {
    create(request_json, password, handshake_ms, true)
}

#[no_mangle]
pub extern "C" fn controller_connection_create_access_v1(
    request_json: *const c_char,
    credential_json: *const c_char,
    timeout_ms: u32,
) -> *mut ControllerSession {
    if credential_json.is_null() {
        return ptr::null_mut();
    }
    let Ok(secret) = (unsafe { CStr::from_ptr(credential_json) }).to_str() else {
        return ptr::null_mut();
    };
    if sodiumoxide::init().is_err() {
        return ptr::null_mut();
    }
    let Some(access) = crate::access::Access::parse(secret) else {
        return ptr::null_mut();
    };
    let task = create(request_json, c"".as_ptr(), timeout_ms, true);
    if let Some(session) = unsafe { task.as_ref() } {
        if session.expected_peer != PersistentPeer::SecureVideo {
            controller_session_destroy(task);
            return ptr::null_mut();
        }
        *session.access.lock().unwrap() = Some(access);
    }
    task
}

fn create(
    request_json: *const c_char,
    password: *const c_char,
    timeout_ms: u32,
    demo: bool,
) -> *mut ControllerSession {
    let max_timeout = if demo { 60_000 } else { 30_000 };
    if request_json.is_null() || password.is_null() || !(100..=max_timeout).contains(&timeout_ms) {
        return ptr::null_mut();
    }
    if sodiumoxide::init().is_err() {
        return ptr::null_mut();
    }
    // SAFETY: The C caller supplies valid NUL-terminated strings for this call.
    let input = unsafe { CStr::from_ptr(request_json) }.to_str();
    let password = unsafe { CStr::from_ptr(password) }.to_str();
    let (Ok(input), Ok(password)) = (input, password) else {
        return ptr::null_mut();
    };
    if password.len() > 512 {
        return ptr::null_mut();
    }
    let Ok(request) = serde_json::from_str::<Request>(input) else {
        return ptr::null_mut();
    };
    if !demo && request.expected_peer.is_some() {
        return ptr::null_mut();
    }
    let mode = request.mode.as_deref().unwrap_or("direct");
    let endpoint = if mode == "direct" {
        let Some(endpoint) = request
            .endpoint
            .as_deref()
            .and_then(|v| v.parse::<SocketAddr>().ok())
            .filter(|v| v.port() != 0)
        else {
            return ptr::null_mut();
        };
        Some(endpoint)
    } else {
        None
    };
    let rendezvous = match mode {
        "direct" => None,
        "id" | "relay" => {
            let Some(config) = crate::rendezvous::Config::parse(
                mode == "relay",
                request.server,
                request.server_key,
                request.relay_server,
            ) else {
                return ptr::null_mut();
            };
            Some(config)
        }
        _ => return ptr::null_mut(),
    };
    if !(6..=20).contains(&request.peer_id.len())
        || !request.peer_id.bytes().all(|b| b.is_ascii_digit())
    {
        return ptr::null_mut();
    }
    if request.minimum_kx_version.unwrap_or(1) != 1 || KX_VERSION_LATEST != 1 {
        return ptr::null_mut();
    }
    let Ok(key) = STANDARD.decode(request.peer_public_key) else {
        return ptr::null_mut();
    };
    let Some(peer_key) = sign::PublicKey::from_slice(&key) else {
        return ptr::null_mut();
    };
    if let Some(fingerprint) = request.peer_fingerprint {
        if fingerprint.len() != 64
            || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
            || format!("{:x}", Sha256::digest(&key)) != fingerprint.to_ascii_lowercase()
        {
            return ptr::null_mut();
        }
    }
    Box::into_raw(Box::new(ControllerSession {
        endpoint,
        rendezvous,
        peer_id: request.peer_id,
        peer_key,
        password: Mutex::new(Some(Zeroizing::new(password.to_owned()))),
        access: Mutex::new(None),
        timeout: Duration::from_millis(timeout_ms.into()),
        started: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        socket: Mutex::new(None),
        demo,
        expected_peer: request.expected_peer.unwrap_or(PersistentPeer::Demo),
        connected: AtomicBool::new(false),
        authorized: AtomicBool::new(false),
        pending: Mutex::new(VecDeque::new()),
        control_input: Mutex::new(ControlInputState::default()),
    }))
}

pub fn approval_code(challenge: &str) -> String {
    let digest = Sha256::digest(challenge.as_bytes());
    let value = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{value:06}")
}

#[no_mangle]
pub extern "C" fn controller_session_send_pointer(
    task: *mut ControllerSession,
    x: u32,
    y: u32,
) -> i32 {
    if x >= 800 || y >= 450 {
        return 3;
    }
    let Some(task) = (unsafe { task.as_ref() }) else {
        return 3;
    };
    queue_input(task, DemoInput::Pointer(x as i32, y as i32))
}

#[no_mangle]
pub extern "C" fn controller_session_send_text(
    task: *mut ControllerSession,
    utf8: *const c_char,
) -> i32 {
    if utf8.is_null() {
        return 3;
    }
    let Ok(text) = (unsafe { CStr::from_ptr(utf8) }).to_str() else {
        return 3;
    };
    if text.is_empty() || text.len() > 512 {
        return 3;
    }
    let Some(task) = (unsafe { task.as_ref() }) else {
        return 3;
    };
    queue_input(task, DemoInput::Text(text.to_owned()))
}

#[no_mangle]
pub extern "C" fn controller_session_send_input_v1(
    task: *mut ControllerSession,
    command_json: *const c_char,
) -> i32 {
    if command_json.is_null() {
        return 3;
    }
    let Ok(raw) = (unsafe { CStr::from_ptr(command_json) }).to_str() else {
        return 3;
    };
    let Ok(command) = parse_command(raw) else {
        return 3;
    };
    let Some(task) = (unsafe { task.as_ref() }) else {
        return 3;
    };
    if task.expected_peer != PersistentPeer::SecureControl
        || !task.connected.load(Ordering::Acquire)
        || task.cancelled.load(Ordering::Acquire)
    {
        return 1;
    }
    let mut input = task.control_input.lock().unwrap();
    if !task.connected.load(Ordering::Acquire) || task.cancelled.load(Ordering::Acquire) {
        return 1;
    }
    input.queue(command)
}

#[no_mangle]
pub extern "C" fn controller_session_set_input_enabled_v1(
    task: *mut ControllerSession,
    enabled: u8,
) -> i32 {
    if enabled > 1 {
        return 3;
    }
    let Some(session) = (unsafe { task.as_ref() }) else {
        return 3;
    };
    if session.expected_peer != PersistentPeer::SecureControl
        || !session.connected.load(Ordering::Acquire)
        || session.cancelled.load(Ordering::Acquire)
    {
        return 1;
    }
    let result = {
        let mut input = session.control_input.lock().unwrap();
        if !session.connected.load(Ordering::Acquire) || session.cancelled.load(Ordering::Acquire) {
            return 1;
        }
        input.request_enabled(enabled == 1)
    };
    if enabled == 0 {
        session.authorized.store(false, Ordering::Release);
    }
    match result {
        Ok(_) => 0,
        Err(code) => {
            if enabled == 0 {
                controller_session_cancel(task);
            }
            code
        }
    }
}

fn queue_input(task: &ControllerSession, input: DemoInput) -> i32 {
    if !task.demo
        || !task.connected.load(Ordering::Acquire)
        || task.cancelled.load(Ordering::Acquire)
    {
        return 1;
    }
    if !task.authorized.load(Ordering::Acquire) {
        return 2;
    }
    if task.expected_peer != PersistentPeer::Demo {
        return 1;
    }
    let mut pending = task.pending.lock().unwrap();
    if !task.connected.load(Ordering::Acquire) || task.cancelled.load(Ordering::Acquire) {
        return 1;
    }
    if !task.authorized.load(Ordering::Acquire) {
        return 2;
    }
    if matches!(input, DemoInput::Pointer(..)) {
        if let Some(DemoInput::Pointer(x, y)) = pending.back_mut() {
            if let DemoInput::Pointer(next_x, next_y) = input {
                *x = next_x;
                *y = next_y;
            }
            return 0;
        }
    }
    if pending.len() >= DEMO_LIMIT {
        return 4;
    }
    pending.push_back(input);
    0
}

pub(crate) struct Failure {
    state: &'static str,
    code: &'static str,
    message: &'static str,
}
impl Failure {
    pub(crate) const fn failed(code: &'static str, message: &'static str) -> Self {
        Self {
            state: "failed",
            code,
            message,
        }
    }
    const fn blocked(code: &'static str, message: &'static str) -> Self {
        Self {
            state: "blocked",
            code,
            message,
        }
    }
}

fn event(
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
    state: &str,
    code: &str,
    message: &str,
    verified: bool,
    authenticated: bool,
) {
    if let Some(callback) = callback {
        let value = json!({"state":state,"code":code,"message":message,"verified":verified,"authenticated":authenticated,"authorized":false});
        if let Ok(value) = CString::new(value.to_string()) {
            // SAFETY: The caller owns callback and user; the JSON pointer is valid during this invocation.
            unsafe { callback(value.as_ptr(), user) }
        }
    }
}

fn demo_event(
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
    state: &str,
    code: &str,
    message: &str,
    verified: bool,
    authenticated: bool,
    authorized: bool,
    extra: serde_json::Value,
) {
    if let Some(callback) = callback {
        let mut value = json!({"state":state,"code":code,"message":message,"verified":verified,
            "authenticated":authenticated,"authorized":authorized});
        if let (Some(fields), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
            fields.extend(extra.clone());
        }
        if let Ok(value) = CString::new(value.to_string()) {
            unsafe { callback(value.as_ptr(), user) }
        }
    }
}

pub(crate) fn status(task: &ControllerSession, deadline: Instant) -> Result<Duration, Failure> {
    if task.cancelled.load(Ordering::Acquire) {
        return Err(Failure {
            state: "cancelled",
            code: "CANCELLED",
            message: "Login verification cancelled",
        });
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Failure::failed("TIMEOUT", "Login verification timed out"));
    }
    Ok(remaining.min(SLICE))
}

pub(crate) fn io_failure(task: &ControllerSession, deadline: Instant) -> Failure {
    status(task, deadline)
        .err()
        .unwrap_or(Failure::failed("TRANSPORT_FAILED", "Connection failed"))
}

pub(crate) struct Wire {
    pub(crate) stream: TcpStream,
    codec: BytesCodec,
    buffer: BytesMut,
}

impl Wire {
    pub(crate) fn new(stream: TcpStream) -> Self {
        let mut codec = BytesCodec::new();
        codec.set_max_packet_length(FRAME_LIMIT);
        Self {
            stream,
            codec,
            buffer: BytesMut::new(),
        }
    }

    pub(crate) fn receive(
        &mut self,
        task: &ControllerSession,
        deadline: Instant,
    ) -> Result<BytesMut, Failure> {
        loop {
            status(task, deadline)?;
            match self.codec.decode(&mut self.buffer) {
                Ok(Some(frame)) => return Ok(frame),
                Err(_) => return Err(Failure::failed("INVALID_FRAME", "Invalid protocol frame")),
                Ok(None) => {}
            }
            let wait = status(task, deadline)?;
            self.stream
                .set_read_timeout(Some(wait))
                .map_err(|_| io_failure(task, deadline))?;
            let mut chunk = [0u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(io_failure(task, deadline)),
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err(io_failure(task, deadline)),
            }
        }
    }

    fn poll_message(
        &mut self,
        task: &ControllerSession,
        deadline: Instant,
        cipher: &mut Encrypt,
    ) -> Result<Option<Message>, Failure> {
        status(task, deadline)?;
        match self.codec.decode(&mut self.buffer) {
            Ok(Some(frame)) => return Self::decode_message(frame, cipher).map(Some),
            Err(_) => return Err(Failure::failed("INVALID_FRAME", "Invalid protocol frame")),
            Ok(None) => {}
        }
        self.stream
            .set_read_timeout(Some(SLICE))
            .map_err(|_| io_failure(task, deadline))?;
        let mut chunk = [0u8; 4096];
        match self.stream.read(&mut chunk) {
            Ok(0) => Err(Failure::failed("DISCONNECTED", "Peer closed connection")),
            Ok(n) => {
                self.buffer.extend_from_slice(&chunk[..n]);
                match self.codec.decode(&mut self.buffer) {
                    Ok(Some(frame)) => Self::decode_message(frame, cipher).map(Some),
                    Ok(None) => Ok(None),
                    Err(_) => Err(Failure::failed("INVALID_FRAME", "Invalid protocol frame")),
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(_) => Err(io_failure(task, deadline)),
        }
    }

    fn decode_message(mut bytes: BytesMut, cipher: &mut Encrypt) -> Result<Message, Failure> {
        let frame_length = bytes.len();
        if bytes.len() < secretbox::MACBYTES || cipher.2 == u64::MAX {
            return Err(Failure::failed(
                "INVALID_CIPHERTEXT",
                "Invalid encrypted message",
            ));
        }
        cipher
            .dec(&mut bytes)
            .map_err(|_| Failure::failed("INVALID_CIPHERTEXT", "Invalid encrypted message"))?;
        let message = Message::parse_from_bytes(&bytes);
        if message.as_ref().map_or(true, |message| {
            matches!(
                &message.union,
                Some(message::message::Union::OrdAccessGrant(_))
            )
        }) {
            bytes.as_mut().zeroize();
        }
        let message =
            message.map_err(|_| Failure::failed("PROTOCOL_FAILED", "Invalid encrypted message"))?;
        if frame_length > FRAME_LIMIT
            && !matches!(&message.union, Some(message::message::Union::VideoFrame(_)))
        {
            return Err(Failure::failed("INVALID_FRAME", "Control packet too large"));
        }
        Ok(message)
    }

    pub(crate) fn send(
        &mut self,
        task: &ControllerSession,
        deadline: Instant,
        bytes: &[u8],
    ) -> Result<(), Failure> {
        let mut framed = BytesMut::new();
        self.codec
            .encode(Bytes::copy_from_slice(bytes), &mut framed)
            .map_err(|_| Failure::failed("INVALID_FRAME", "Protocol frame too large"))?;
        let mut remaining = framed.as_ref();
        while !remaining.is_empty() {
            let wait = status(task, deadline)?;
            self.stream
                .set_write_timeout(Some(wait))
                .map_err(|_| io_failure(task, deadline))?;
            match self.stream.write(remaining) {
                Ok(0) => return Err(io_failure(task, deadline)),
                Ok(n) => remaining = &remaining[n..],
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err(io_failure(task, deadline)),
            }
        }
        Ok(())
    }

    fn send_message(
        &mut self,
        task: &ControllerSession,
        deadline: Instant,
        cipher: &mut Encrypt,
        message: &Message,
    ) -> Result<(), Failure> {
        if cipher.1 == u64::MAX {
            return Err(Failure::failed(
                "NONCE_EXHAUSTED",
                "Encrypted session counter exhausted",
            ));
        }
        let plain = Zeroizing::new(
            message
                .write_to_bytes()
                .map_err(|_| Failure::failed("PROTOCOL_FAILED", "Cannot encode login message"))?,
        );
        self.send(task, deadline, &cipher.enc(&plain))
    }

    fn receive_message(
        &mut self,
        task: &ControllerSession,
        deadline: Instant,
        cipher: &mut Encrypt,
    ) -> Result<Message, Failure> {
        let mut bytes = self.receive(task, deadline)?;
        if bytes.len() < secretbox::MACBYTES || cipher.2 == u64::MAX {
            return Err(Failure::failed(
                "INVALID_CIPHERTEXT",
                "Invalid encrypted message",
            ));
        }
        cipher
            .dec(&mut bytes)
            .map_err(|_| Failure::failed("INVALID_CIPHERTEXT", "Invalid encrypted message"))?;
        let message = Message::parse_from_bytes(&bytes);
        if message.as_ref().map_or(true, |message| {
            matches!(
                &message.union,
                Some(message::message::Union::OrdAccessGrant(_))
            )
        }) {
            bytes.as_mut().zeroize();
        }
        message.map_err(|_| Failure::failed("PROTOCOL_FAILED", "Invalid login message"))
    }
}

fn response(
    password: &mut Zeroizing<String>,
    challenge: &Hash,
    peer_id: &str,
    demo: bool,
    video: bool,
    control: bool,
) -> Message {
    let digest = if password.is_empty() {
        Vec::new()
    } else {
        let first = Zeroizing::new(
            Sha256::new()
                .chain_update(password.as_bytes())
                .chain_update(challenge.salt.as_bytes())
                .finalize()
                .to_vec(),
        );
        let second = Zeroizing::new(
            Sha256::new()
                .chain_update(first.as_slice())
                .chain_update(challenge.challenge.as_bytes())
                .finalize()
                .to_vec(),
        );
        second.to_vec()
    };
    password.zeroize();
    let option = OptionMessage {
        disable_keyboard: (if demo && (!video || control) {
            option_message::BoolOption::No
        } else {
            option_message::BoolOption::Yes
        })
        .into(),
        disable_audio: option_message::BoolOption::Yes.into(),
        disable_clipboard: option_message::BoolOption::Yes.into(),
        disable_camera: option_message::BoolOption::Yes.into(),
        enable_file_transfer: option_message::BoolOption::No.into(),
        block_input: option_message::BoolOption::No.into(),
        privacy_mode: option_message::BoolOption::No.into(),
        supported_decoding: if video {
            MessageField::some(SupportedDecoding {
                ability_vp8: 1,
                prefer: message::supported_decoding::PreferCodec::VP8.into(),
                ..Default::default()
            })
        } else {
            MessageField::none()
        },
        ..Default::default()
    };
    let mut message = Message::new();
    message.set_login_request(LoginRequest {
        username: peer_id.to_owned(),
        password: digest,
        my_id: "123456789".to_owned(),
        my_name: "Harmony Controller".to_owned(),
        ord_input_version: if control { 2 } else { 0 },
        option: MessageField::some(option),
        ..Default::default()
    });
    message
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistentPeer {
    Demo,
    SecureHost,
    SecureVideo,
    SecureControl,
}

fn video_dimensions(peer: &message::PeerInfo) -> Option<(u32, u32)> {
    let value: serde_json::Value = serde_json::from_str(&peer.platform_additions).ok()?;
    let fields = value.as_object()?;
    if fields.len() != 4
        || fields.get("ord_secure_host")?.as_u64()? != 1
        || fields.get("media")?.as_bool()? != true
        || fields.get("video_codec")?.as_str()? != "vp8"
        || fields.get("input_scope")?.as_str()? != "none"
        || peer.displays.len() != 1
        || peer.current_display != 0
    {
        return None;
    }
    let display = &peer.displays[0];
    if !(1..=1280).contains(&display.width) || !(1..=720).contains(&display.height) {
        return None;
    }
    Some((display.width as u32, display.height as u32))
}

fn control_video_dimensions(peer: &message::PeerInfo) -> Option<(u32, u32)> {
    let value: serde_json::Value = serde_json::from_str(&peer.platform_additions).ok()?;
    let fields = value.as_object()?;
    if fields.len() != 5
        || fields.get("ord_secure_host")?.as_u64()? != 1
        || fields.get("media")?.as_bool()? != true
        || fields.get("video_codec")?.as_str()? != "vp8"
        || fields.get("input_scope")?.as_str()? != "windows_primary"
        || fields.get("input_version")?.as_u64()? != 2
        || peer.displays.len() != 1
        || peer.current_display != 0
        || !peer.displays[0].online
    {
        return None;
    }
    let display = &peer.displays[0];
    if !(1..=1280).contains(&display.width) || !(1..=720).contains(&display.height) {
        return None;
    }
    Some((display.width as u32, display.height as u32))
}

fn run_secure_video(
    task: &ControllerSession,
    callback: Option<ControllerSessionCallback>,
    video_callback: ControllerVideoCallback,
    user: *mut c_void,
    wire: &mut Wire,
    cipher: &mut Encrypt,
    width: u32,
    height: u32,
) -> Result<(), Failure> {
    let deadline = Instant::now() + DEMO_LIFETIME;
    let mut first_frame = true;
    let mut frames = 0u64;
    let mut bytes = 0u64;
    let mut last_status = Instant::now();
    loop {
        if let Some(message) = wire.poll_message(task, deadline, cipher)? {
            match message.union {
                Some(message::message::Union::VideoFrame(video)) => {
                    let Some(message::video_frame::Union::Vp8s(vp8)) = video.union else {
                        return Err(Failure::failed("INVALID_VIDEO", "Unsupported video codec"));
                    };
                    if video.display != 0 || !(1..=4).contains(&vp8.frames.len()) {
                        return Err(Failure::failed(
                            "INVALID_VIDEO",
                            "Invalid video display or frame count",
                        ));
                    }
                    let mut total = 0usize;
                    for frame in &vp8.frames {
                        if frame.data.is_empty() || frame.data.len() > VIDEO_PART_LIMIT {
                            return Err(Failure::failed(
                                "INVALID_VIDEO",
                                "Invalid encoded video size",
                            ));
                        }
                        total = total.saturating_add(frame.data.len());
                        if total > VIDEO_FRAME_LIMIT || first_frame && !frame.key {
                            return Err(Failure::failed(
                                "INVALID_VIDEO",
                                "Invalid initial or oversized video frame",
                            ));
                        }
                        first_frame = false;
                    }
                    for frame in &vp8.frames {
                        status(task, deadline)?;
                        // SAFETY: The borrowed frame bytes remain alive through this synchronous call.
                        unsafe {
                            video_callback(
                                frame.data.as_ptr(),
                                frame.data.len() as u32,
                                width,
                                height,
                                frame.pts,
                                frame.key as u8,
                                user,
                            )
                        };
                        frames += 1;
                        bytes += frame.data.len() as u64;
                        status(task, deadline)?;
                    }
                    if last_status.elapsed() >= Duration::from_secs(1) {
                        demo_event(
                            callback,
                            user,
                            "video_status",
                            "VIDEO_STATUS",
                            "Video streaming",
                            true,
                            true,
                            false,
                            json!({"frames":frames,"bytes":bytes}),
                        );
                        last_status = Instant::now();
                    }
                }
                Some(message::message::Union::Misc(misc)) => match misc.union {
                    Some(message::misc::Union::PermissionInfo(permission))
                        if permission.enabled =>
                    {
                        return Err(Failure::failed(
                            "UNEXPECTED_PERMISSION",
                            "Read-only video cannot grant input",
                        ));
                    }
                    Some(message::misc::Union::PermissionInfo(_)) => {}
                    Some(message::misc::Union::CloseReason(_)) => {
                        return Err(Failure::failed(
                            "DISCONNECTED",
                            "Peer closed video connection",
                        ));
                    }
                    _ => {
                        return Err(Failure::failed(
                            "UNSUPPORTED_MESSAGE",
                            "Unsupported video message",
                        ))
                    }
                },
                Some(message::message::Union::TestDelay(_)) => {}
                _ => {
                    return Err(Failure::failed(
                        "UNSUPPORTED_MESSAGE",
                        "Unsupported video message",
                    ))
                }
            }
        }
    }
}

fn run_secure_control(
    task: &ControllerSession,
    callback: Option<ControllerSessionCallback>,
    video_callback: ControllerVideoCallback,
    user: *mut c_void,
    wire: &mut Wire,
    cipher: &mut Encrypt,
    width: u32,
    height: u32,
) -> Result<(), Failure> {
    let deadline = Instant::now() + DEMO_LIFETIME;
    let mut first_frame = true;
    let mut frames = 0u64;
    let mut bytes = 0u64;
    let mut last_status = Instant::now();
    let mut last_heartbeat = Instant::now();
    let mut last_inbound = Instant::now();
    loop {
        if let Some(message) = wire.poll_message(task, deadline, cipher)? {
            if message
                .special_fields
                .unknown_fields()
                .iter()
                .next()
                .is_some()
            {
                return Err(Failure::failed(
                    "UNSUPPORTED_MESSAGE",
                    "Unknown control message",
                ));
            }
            let mut refresh_inbound = true;
            match message.union {
                Some(message::message::Union::VideoFrame(video)) => {
                    let Some(message::video_frame::Union::Vp8s(vp8)) = video.union else {
                        return Err(Failure::failed("INVALID_VIDEO", "Unsupported video codec"));
                    };
                    if video.display != 0 || !(1..=4).contains(&vp8.frames.len()) {
                        return Err(Failure::failed(
                            "INVALID_VIDEO",
                            "Invalid video display or frame count",
                        ));
                    }
                    let mut total = 0usize;
                    for frame in &vp8.frames {
                        if frame.data.is_empty() || frame.data.len() > VIDEO_PART_LIMIT {
                            return Err(Failure::failed(
                                "INVALID_VIDEO",
                                "Invalid encoded video size",
                            ));
                        }
                        total = total.saturating_add(frame.data.len());
                        if total > VIDEO_FRAME_LIMIT || first_frame && !frame.key {
                            return Err(Failure::failed(
                                "INVALID_VIDEO",
                                "Invalid initial or oversized video frame",
                            ));
                        }
                        first_frame = false;
                    }
                    for frame in &vp8.frames {
                        status(task, deadline)?;
                        // SAFETY: Frame bytes remain borrowed through this synchronous callback.
                        unsafe {
                            video_callback(
                                frame.data.as_ptr(),
                                frame.data.len() as u32,
                                width,
                                height,
                                frame.pts,
                                frame.key as u8,
                                user,
                            )
                        };
                        frames += 1;
                        bytes += frame.data.len() as u64;
                        status(task, deadline)?;
                    }
                    if last_status.elapsed() >= Duration::from_secs(1) {
                        let input = task.control_input.lock().unwrap();
                        let supported = input.supported();
                        let allowed = input.allowed();
                        drop(input);
                        demo_event(
                            callback,
                            user,
                            "video_status",
                            "VIDEO_STATUS",
                            "Video streaming",
                            true,
                            true,
                            allowed,
                            json!({"frames":frames,"bytes":bytes,"inputSupported":supported}),
                        );
                        last_status = Instant::now();
                    }
                }
                Some(message::message::Union::OrdInputState(state)) => {
                    let state_result = {
                        let mut input = task.control_input.lock().unwrap();
                        let result = input.apply_state(&state);
                        task.authorized.store(input.allowed(), Ordering::Release);
                        result
                    }
                    .map_err(|_| {
                        Failure::failed("INVALID_INPUT_STATE", "Invalid input permission state")
                    })?;
                    if let Some((supported, allowed)) = state_result {
                        demo_event(
                            callback,
                            user,
                            "input_state",
                            "INPUT_STATE",
                            "Input permission changed",
                            true,
                            true,
                            allowed,
                            json!({"inputSupported":supported}),
                        );
                    }
                }
                Some(message::message::Union::OrdInputEvent(event)) => {
                    let valid_echo = event.version == 1
                        && event.grant_token.len() == 16
                        && event
                            .special_fields
                            .unknown_fields()
                            .iter()
                            .next()
                            .is_none()
                        && matches!(event.command,
                            Some(message::ord_input_event::Command::KeepAlive(ref heartbeat))
                            if heartbeat.special_fields.unknown_fields().iter().next().is_none());
                    let mut token = [0; 16];
                    if valid_echo {
                        token.copy_from_slice(&event.grant_token);
                    }
                    if !valid_echo {
                        return Err(Failure::failed(
                            "INVALID_INPUT_HEARTBEAT",
                            "Invalid input heartbeat echo",
                        ));
                    }
                    if !task.control_input.lock().unwrap().matches_token(token) {
                        refresh_inbound = false;
                    }
                }
                Some(message::message::Union::Misc(misc)) => match misc.union {
                    Some(message::misc::Union::PermissionInfo(permission))
                        if permission.enabled
                            && permission.permission.enum_value()
                                != Ok(permission_info::Permission::Keyboard) =>
                    {
                        return Err(Failure::failed(
                            "UNEXPECTED_PERMISSION",
                            "Legacy permission cannot grant control",
                        ));
                    }
                    Some(message::misc::Union::PermissionInfo(_)) => {}
                    Some(message::misc::Union::CloseReason(reason)) => {
                        if reason == "Input release failed" {
                            return Err(Failure::failed(
                                "INPUT_RELEASE_FAILED",
                                "Peer could not release injected input",
                            ));
                        }
                        return Err(Failure::failed(
                            "DISCONNECTED",
                            "Peer closed control connection",
                        ));
                    }
                    _ => {
                        return Err(Failure::failed(
                            "UNSUPPORTED_MESSAGE",
                            "Unsupported control message",
                        ))
                    }
                },
                Some(message::message::Union::TestDelay(_)) => {}
                _ => {
                    return Err(Failure::failed(
                        "UNSUPPORTED_MESSAGE",
                        "Unsupported control message",
                    ))
                }
            }
            if refresh_inbound {
                last_inbound = Instant::now();
            }
        }
        if task.control_input.lock().unwrap().allowed()
            && last_inbound.elapsed() >= Duration::from_secs(5)
        {
            return Err(Failure::failed(
                "TRANSPORT_STALLED",
                "Control peer stopped responding",
            ));
        }
        let request = { task.control_input.lock().unwrap().pop_request() };
        if let Some(request) = request {
            let mut message = Message::new();
            message.set_ord_input_request(request);
            let send_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
            wire.send_message(task, send_deadline, cipher, &message)?;
        }
        for _ in 0..8 {
            let next_input = { task.control_input.lock().unwrap().pop() };
            let Some((command, token)) = next_input else {
                break;
            };
            if task.control_input.lock().unwrap().matches_token(token) {
                let send_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
                wire.send_message(task, send_deadline, cipher, &command.into_message(token))?;
            }
        }
        if last_heartbeat.elapsed() >= Duration::from_secs(1) {
            let heartbeat = { task.control_input.lock().unwrap().keep_alive_message() };
            if let Some(heartbeat) = heartbeat {
                let send_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
                wire.send_message(task, send_deadline, cipher, &heartbeat)?;
            }
            last_heartbeat = Instant::now();
        }
    }
}

fn persistent_peer(additions: &str) -> Option<PersistentPeer> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(additions) else {
        return None;
    };
    if value.get("ord_demo").is_some() && value.get("ord_secure_host").is_some() {
        return None;
    }
    if value.get("ord_demo").and_then(|v| v.as_u64()) == Some(1)
        && value.get("input_scope").and_then(|v| v.as_str()) == Some("demo_window")
        && value.get("width").and_then(|v| v.as_u64()) == Some(800)
        && value.get("height").and_then(|v| v.as_u64()) == Some(450)
    {
        return Some(PersistentPeer::Demo);
    }
    if value.get("ord_secure_host").and_then(|v| v.as_u64()) == Some(1)
        && value.get("media").and_then(|v| v.as_bool()) == Some(false)
        && value.get("input_scope").and_then(|v| v.as_str()) == Some("none")
    {
        return Some(PersistentPeer::SecureHost);
    }
    None
}

fn run_secure_host(
    task: &ControllerSession,
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
    wire: &mut Wire,
    cipher: &mut Encrypt,
) -> Result<(), Failure> {
    let deadline = Instant::now() + DEMO_LIFETIME;
    loop {
        if let Some(message) = wire.poll_message(task, deadline, cipher)? {
            match message.union {
                Some(message::message::Union::Misc(misc)) => match misc.union {
                    Some(message::misc::Union::PermissionInfo(permission)) => {
                        if permission.enabled {
                            return Err(Failure::failed(
                                "UNEXPECTED_PERMISSION",
                                "Formal secure entry cannot grant capabilities in this phase",
                            ));
                        }
                        demo_event(
                            callback,
                            user,
                            "permissions_changed",
                            "PERMISSIONS_CHANGED",
                            "Formal secure entry remains read-only",
                            true,
                            true,
                            false,
                            json!({}),
                        );
                    }
                    Some(message::misc::Union::CloseReason(_)) => {
                        return Err(Failure::failed("DISCONNECTED", "Peer closed secure entry"));
                    }
                    _ => {
                        return Err(Failure::failed(
                            "UNSUPPORTED_MESSAGE",
                            "Unsupported secure entry message",
                        ));
                    }
                },
                Some(message::message::Union::TestDelay(_)) => {}
                _ => {
                    return Err(Failure::failed(
                        "UNSUPPORTED_MESSAGE",
                        "Unsupported secure entry message",
                    ));
                }
            }
        }
    }
}

fn run_demo(
    task: &ControllerSession,
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
    wire: &mut Wire,
    cipher: &mut Encrypt,
) -> Result<(), Failure> {
    let deadline = Instant::now() + DEMO_LIFETIME;
    loop {
        if let Some(message) = wire.poll_message(task, deadline, cipher)? {
            match message.union {
                Some(message::message::Union::Misc(misc)) => match misc.union {
                    Some(message::misc::Union::PermissionInfo(permission)) => {
                        if permission.permission.enum_value()
                            == Ok(permission_info::Permission::Keyboard)
                        {
                            let allowed = permission.enabled;
                            {
                                let mut pending = task.pending.lock().unwrap();
                                if task.cancelled.load(Ordering::Acquire) {
                                    return Err(io_failure(task, deadline));
                                }
                                task.authorized.store(allowed, Ordering::Release);
                                if !allowed {
                                    pending.clear();
                                }
                            }
                            demo_event(
                                callback,
                                user,
                                "permissions_changed",
                                "PERMISSIONS_CHANGED",
                                "Demo input permission changed",
                                true,
                                true,
                                allowed,
                                json!({}),
                            );
                        }
                    }
                    Some(message::misc::Union::ChatMessage(chat)) => {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&chat.text) {
                            let x = value.get("x").and_then(|v| v.as_u64());
                            let y = value.get("y").and_then(|v| v.as_u64());
                            let text_length = value.get("textLength").and_then(|v| v.as_u64());
                            if value.get("ord_demo_status").and_then(|v| v.as_u64()) == Some(1)
                                && x.is_some_and(|n| n < 800)
                                && y.is_some_and(|n| n < 450)
                                && text_length.is_some()
                            {
                                demo_event(
                                    callback,
                                    user,
                                    "demo_status",
                                    "DEMO_STATUS",
                                    "Demo status updated",
                                    true,
                                    true,
                                    task.authorized.load(Ordering::Acquire),
                                    json!({"x":x,"y":y,"textLength":text_length}),
                                );
                            }
                        }
                    }
                    Some(message::misc::Union::CloseReason(_)) => {
                        return Err(Failure::failed(
                            "DISCONNECTED",
                            "Peer closed demo connection",
                        ))
                    }
                    _ => {
                        return Err(Failure::failed(
                            "UNSUPPORTED_MESSAGE",
                            "Unsupported demo message",
                        ))
                    }
                },
                Some(message::message::Union::TestDelay(_)) => {}
                _ => {
                    return Err(Failure::failed(
                        "UNSUPPORTED_MESSAGE",
                        "Unsupported demo message",
                    ))
                }
            }
        }
        if !task.authorized.load(Ordering::Acquire) {
            continue;
        }
        let input = task.pending.lock().unwrap().pop_front();
        if let Some(input) = input {
            if !task.authorized.load(Ordering::Acquire) || task.cancelled.load(Ordering::Acquire) {
                continue;
            }
            let mut message = Message::new();
            match input {
                DemoInput::Pointer(x, y) => message.set_mouse_event(MouseEvent {
                    mask: 0,
                    x,
                    y,
                    ..Default::default()
                }),
                DemoInput::Text(text) => {
                    let mut key = KeyEvent::new();
                    key.set_seq(text);
                    message.set_key_event(key);
                }
            }
            wire.send_message(task, deadline, cipher, &message)?;
        }
    }
}

fn run(
    task: &ControllerSession,
    callback: Option<ControllerSessionCallback>,
    video_callback: Option<ControllerVideoCallback>,
    video_entry: bool,
    user: *mut c_void,
) {
    let deadline = Instant::now() + task.timeout;
    let mut verified = false;
    let result = (|| -> Result<(), Failure> {
        status(task, deadline)?;
        if video_entry
            && !matches!(
                task.expected_peer,
                PersistentPeer::SecureVideo | PersistentPeer::SecureControl
            )
        {
            return Err(Failure::failed(
                "VIDEO_MODE_REQUIRED",
                "Video entry requires secure_video",
            ));
        }
        if matches!(
            task.expected_peer,
            PersistentPeer::SecureVideo | PersistentPeer::SecureControl
        ) && video_callback.is_none()
        {
            return Err(Failure::failed(
                "VIDEO_SINK_REQUIRED",
                "Video callback required",
            ));
        }
        event(
            callback,
            user,
            "connecting",
            "CONNECTING",
            "Connecting to peer",
            false,
            false,
        );
        let (mut wire, connection_path) = if let Some(config) = &task.rendezvous {
            crate::rendezvous::connect(task, deadline, config)?
        } else {
            let connect_wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(2));
            if connect_wait.is_zero() {
                return Err(Failure::failed("TIMEOUT", "Login verification timed out"));
            }
            let endpoint = task.endpoint.ok_or(Failure::failed(
                "INVALID_ENDPOINT",
                "Missing direct endpoint",
            ))?;
            let stream = TcpStream::connect_timeout(&endpoint, connect_wait)
                .map_err(|_| io_failure(task, deadline))?;
            let clone = stream.try_clone().map_err(|_| io_failure(task, deadline))?;
            *task.socket.lock().unwrap() = Some(clone);
            status(task, deadline)?;
            (Wire::new(stream), "direct")
        };
        demo_event(
            callback,
            user,
            "transport_selected",
            "TRANSPORT_SELECTED",
            "Peer transport selected",
            false,
            false,
            false,
            json!({"connectionPath": connection_path}),
        );
        let first = wire.receive(task, deadline)?;
        let signed = match Message::parse_from_bytes(&first)
            .ok()
            .and_then(|message| message.union)
        {
            Some(message::message::Union::SignedId(signed)) => signed,
            _ => {
                return Err(Failure::failed(
                    "IDENTITY_REQUIRED",
                    "Peer identity is missing",
                ))
            }
        };
        let (id, peer_ephemeral, _, advertised) =
            upstream_crypto::decode_id_pk_dtls(&signed.id, &task.peer_key)
                .map_err(|_| Failure::failed("IDENTITY_INVALID", "Peer signature is invalid"))?;
        if id != task.peer_id {
            return Err(Failure::failed(
                "PEER_ID_MISMATCH",
                "Peer ID does not match",
            ));
        }
        if advertised < 1 {
            return Err(Failure::failed(
                "KX_VERSION_UNSUPPORTED",
                "Peer does not support encrypted key exchange v1",
            ));
        }
        verified = true;
        event(
            callback,
            user,
            "verified",
            "IDENTITY_VERIFIED",
            "Peer identity verified",
            true,
            false,
        );
        let (our_ephemeral, sealed, key) =
            upstream_crypto::create_symmetric_key_msg(peer_ephemeral);
        let mut public = Message::new();
        public.set_public_key(PublicKey {
            asymmetric_value: our_ephemeral.to_vec(),
            symmetric_value: sealed.to_vec(),
            kx_version: 1,
            ..Default::default()
        });
        wire.send(
            task,
            deadline,
            &public
                .write_to_bytes()
                .map_err(|_| Failure::failed("PROTOCOL_FAILED", "Cannot encode key exchange"))?,
        )?;
        let mut cipher = Encrypt::new_split(
            key,
            true,
            &KxTranscript {
                initiator_pk: &our_ephemeral,
                responder_pk: &peer_ephemeral,
                advertised,
                picked: 1,
            },
        )
        .map_err(|_| {
            Failure::failed("KEY_EXCHANGE_FAILED", "Cannot establish encrypted session")
        })?;
        let hash = match wire.receive_message(task, deadline, &mut cipher)?.union {
            Some(message::message::Union::Hash(hash)) => hash,
            _ => {
                return Err(Failure::failed(
                    "PROTOCOL_FAILED",
                    "Expected encrypted login challenge",
                ))
            }
        };
        if hash.challenge.is_empty() {
            return Err(Failure::failed(
                "PROTOCOL_FAILED",
                "Invalid login challenge",
            ));
        }
        let mut access = task.access.lock().unwrap().take();
        let mut password = task.password.lock().unwrap().take().ok_or(Failure::failed(
            "PROTOCOL_FAILED",
            "Missing login credential",
        ))?;
        let awaiting_approval = password.is_empty()
            && access
                .as_ref()
                .is_none_or(|access| access.mode != "unattended");
        let mut login = response(
            &mut password,
            &hash,
            &task.peer_id,
            task.demo,
            matches!(
                task.expected_peer,
                PersistentPeer::SecureVideo | PersistentPeer::SecureControl
            ),
            task.expected_peer == PersistentPeer::SecureControl,
        );
        if let Some(access) = access.as_mut() {
            let proof = access
                .proof(&task.peer_id, &task.peer_key, &hash.challenge)
                .map_err(|code| Failure::failed(code, "Trusted access proof failed"))?;
            if let Some(message::message::Union::LoginRequest(request)) = login.union.as_mut() {
                request.ord_access = MessageField::some(proof);
            }
        }
        let send_result = wire.send_message(task, deadline, &mut cipher, &login);
        if let Some(message::message::Union::LoginRequest(request)) = login.union.as_mut() {
            request.password.zeroize();
            if let Some(access) = request.ord_access.as_mut() {
                access.credential_secret.zeroize();
            }
        }
        send_result?;
        if task.demo && awaiting_approval {
            demo_event(
                callback,
                user,
                "awaiting_approval",
                "AWAITING_APPROVAL",
                "Waiting for peer approval",
                true,
                false,
                false,
                json!({"confirmationCode": approval_code(&hash.challenge)}),
            );
        } else {
            event(
                callback,
                user,
                if awaiting_approval {
                    "awaiting_approval"
                } else {
                    "authenticating"
                },
                if awaiting_approval {
                    "AWAITING_APPROVAL"
                } else {
                    "AUTHENTICATING"
                },
                if awaiting_approval {
                    "Waiting for peer approval"
                } else {
                    "Waiting for login result"
                },
                true,
                false,
            );
        }
        loop {
            let message = wire.receive_message(task, deadline, &mut cipher)?;
            match message.union {
                Some(message::message::Union::OrdAccessGrant(mut grant)) => {
                    let result = access
                        .as_mut()
                        .ok_or("ACCESS_GRANT_INVALID")
                        .and_then(|access| access.grant(&grant));
                    if result.is_ok() {
                        crate::access::paired_event(callback, user, &grant);
                    }
                    grant.credential_secret.zeroize();
                    result.map_err(|code| Failure::failed(code, "Invalid trusted access grant"))?;
                }
                Some(message::message::Union::LoginResponse(login)) => match login.union {
                    Some(login_response::Union::PeerInfo(mut peer)) => {
                        if let Some(access) = &access {
                            access
                                .verify_peer_info(&peer.platform_additions)
                                .map_err(|code| {
                                    Failure::failed(code, "Trusted access mode was not confirmed")
                                })?;
                            let mut fields: serde_json::Value =
                                serde_json::from_str(&peer.platform_additions).map_err(|_| {
                                    Failure::failed(
                                        "ACCESS_MODE_MISMATCH",
                                        "Invalid trusted peer metadata",
                                    )
                                })?;
                            if let Some(fields) = fields.as_object_mut() {
                                fields.remove("access_mode");
                            }
                            peer.platform_additions = fields.to_string();
                        }
                        if task.demo {
                            let video_size = match task.expected_peer {
                                PersistentPeer::SecureVideo => video_dimensions(&peer),
                                PersistentPeer::SecureControl => control_video_dimensions(&peer),
                                _ => None,
                            };
                            let Some(peer_kind) = persistent_peer(&peer.platform_additions)
                                .filter(|kind| *kind == task.expected_peer)
                                .or_else(|| match task.expected_peer {
                                    PersistentPeer::SecureVideo | PersistentPeer::SecureControl => {
                                        video_size.map(|_| task.expected_peer)
                                    }
                                    _ => None,
                                })
                            else {
                                let mut misc = Misc::new();
                                misc.set_close_reason("Unsupported persistent peer".to_owned());
                                let mut close = Message::new();
                                close.set_misc(misc);
                                let _ = wire.send_message(task, deadline, &mut cipher, &close);
                                return Err(Failure::failed(
                                    "UNSUPPORTED_PEER",
                                    "Peer does not support the requested persistent mode",
                                ));
                            };
                            {
                                let _pending = task.pending.lock().unwrap();
                                status(task, deadline)?;
                                task.connected.store(true, Ordering::Release);
                            }
                            let message = if peer_kind == PersistentPeer::Demo {
                                "Demo connection established"
                            } else if peer_kind == PersistentPeer::SecureControl {
                                "Control-capable video connection established; input awaits local permission"
                            } else if peer_kind == PersistentPeer::SecureVideo {
                                "Read-only video connection established"
                            } else {
                                "Formal secure entry established; read-only"
                            };
                            demo_event(
                                callback,
                                user,
                                "connected",
                                "CONNECTED",
                                message,
                                true,
                                true,
                                false,
                                if let Some((width, height)) = video_size {
                                    if peer_kind == PersistentPeer::SecureControl {
                                        json!({"videoWidth":width,"videoHeight":height,"videoCodec":"vp8","inputSupported":false})
                                    } else {
                                        if let Some(access) = &access {
                                            json!({"videoWidth":width,"videoHeight":height,"videoCodec":"vp8","accessMode":access.mode})
                                        } else {
                                            json!({"videoWidth":width,"videoHeight":height,"videoCodec":"vp8"})
                                        }
                                    }
                                } else {
                                    json!({})
                                },
                            );
                            return if peer_kind == PersistentPeer::Demo {
                                run_demo(task, callback, user, &mut wire, &mut cipher)
                            } else if peer_kind == PersistentPeer::SecureVideo {
                                wire.codec.set_max_packet_length(VIDEO_FRAME_LIMIT);
                                let (width, height) = video_size.ok_or(Failure::failed(
                                    "UNSUPPORTED_PEER",
                                    "Invalid video peer",
                                ))?;
                                run_secure_video(
                                    task,
                                    callback,
                                    video_callback.ok_or(Failure::failed(
                                        "VIDEO_SINK_REQUIRED",
                                        "Video callback required",
                                    ))?,
                                    user,
                                    &mut wire,
                                    &mut cipher,
                                    width,
                                    height,
                                )
                            } else if peer_kind == PersistentPeer::SecureControl {
                                wire.codec.set_max_packet_length(VIDEO_FRAME_LIMIT);
                                let (width, height) = video_size.ok_or(Failure::failed(
                                    "UNSUPPORTED_PEER",
                                    "Invalid control video peer",
                                ))?;
                                run_secure_control(
                                    task,
                                    callback,
                                    video_callback.ok_or(Failure::failed(
                                        "VIDEO_SINK_REQUIRED",
                                        "Video callback required",
                                    ))?,
                                    user,
                                    &mut wire,
                                    &mut cipher,
                                    width,
                                    height,
                                )
                            } else {
                                run_secure_host(task, callback, user, &mut wire, &mut cipher)
                            };
                        }
                        event(
                            callback,
                            user,
                            "authentication_confirmed",
                            "AUTHENTICATED",
                            "Peer confirmed login",
                            true,
                            true,
                        );
                        let mut misc = Misc::new();
                        misc.set_close_reason("Login verification complete".to_owned());
                        let mut close = Message::new();
                        close.set_misc(misc);
                        let _ = wire.send_message(task, deadline, &mut cipher, &close);
                        return Ok(());
                    }
                    Some(login_response::Union::Error(error)) if error == "2FA Required" => {
                        return Err(Failure::blocked(
                            "SECOND_FACTOR_REQUIRED",
                            "Second factor required",
                        ))
                    }
                    Some(login_response::Union::Error(error))
                        if error == "Wrong Password" || error == "Empty Password" =>
                    {
                        return Err(Failure::failed("PASSWORD_REJECTED", "Password rejected"))
                    }
                    _ => return Err(Failure::failed("LOGIN_REJECTED", "Peer rejected login")),
                },
                Some(message::message::Union::TestDelay(_)) => {}
                Some(message::message::Union::Misc(misc))
                    if matches!(misc.union, Some(message::misc::Union::PermissionInfo(_))) =>
                {
                    if task.demo
                        && task.expected_peer != PersistentPeer::Demo
                        && matches!(misc.union, Some(message::misc::Union::PermissionInfo(p)) if p.enabled)
                    {
                        return Err(Failure::failed(
                            "UNEXPECTED_PERMISSION",
                            "Formal secure entry cannot grant capabilities in this phase",
                        ));
                    }
                }
                Some(message::message::Union::VideoFrame(_))
                    if task.expected_peer == PersistentPeer::SecureVideo =>
                {
                    return Err(Failure::failed("UNEXPECTED_VIDEO", "Video before approval"));
                }
                _ => {
                    return Err(Failure::failed(
                        "PROTOCOL_FAILED",
                        "Unexpected login message",
                    ))
                }
            }
        }
    })();
    {
        let mut pending = task.pending.lock().unwrap();
        task.connected.store(false, Ordering::Release);
        task.authorized.store(false, Ordering::Release);
        pending.clear();
    }
    task.control_input.lock().unwrap().clear();
    task.password.lock().unwrap().take();
    task.access.lock().unwrap().take();
    *task.socket.lock().unwrap() = None;
    if let Err(error) = result {
        if task.demo {
            demo_event(
                callback,
                user,
                error.state,
                error.code,
                error.message,
                verified,
                false,
                false,
                json!({}),
            );
        } else {
            event(
                callback,
                user,
                error.state,
                error.code,
                error.message,
                verified,
                false,
            );
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_session_run(
    task: *mut ControllerSession,
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
) {
    if let Some(task) = unsafe { task.as_ref() } {
        if !task.started.swap(true, Ordering::AcqRel) {
            run(task, callback, None, false, user)
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_session_run_video(
    task: *mut ControllerSession,
    callback: Option<ControllerSessionCallback>,
    video_callback: Option<ControllerVideoCallback>,
    user: *mut c_void,
) {
    if let Some(task) = unsafe { task.as_ref() } {
        if !task.started.swap(true, Ordering::AcqRel) {
            run(task, callback, video_callback, true, user)
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_session_cancel(task: *mut ControllerSession) {
    if let Some(task) = unsafe { task.as_ref() } {
        {
            let mut pending = task.pending.lock().unwrap();
            task.cancelled.store(true, Ordering::Release);
            task.connected.store(false, Ordering::Release);
            task.authorized.store(false, Ordering::Release);
            pending.clear();
        }
        task.control_input.lock().unwrap().clear();
        task.password.lock().unwrap().take();
        task.access.lock().unwrap().take();
        if let Some(socket) = task.socket.lock().unwrap().as_ref() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_session_destroy(task: *mut ControllerSession) {
    if !task.is_null() {
        // SAFETY: The caller destroys each task once and waits for run to return.
        unsafe { drop(Box::from_raw(task)) }
    }
}
