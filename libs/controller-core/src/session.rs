use std::{
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
    protos::message::{
        self, login_response, option_message, Hash, LoginRequest, Message, Misc, OptionMessage,
        PublicKey,
    },
    upstream_crypto::{self, Encrypt, KxTranscript, KX_VERSION_LATEST},
};

const FRAME_LIMIT: usize = 64 * 1024;
const SLICE: Duration = Duration::from_millis(100);

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Request {
    endpoint: String,
    peer_id: String,
    peer_public_key: String,
    peer_fingerprint: Option<String>,
    minimum_kx_version: Option<u32>,
}

pub struct ControllerSession {
    endpoint: SocketAddr,
    peer_id: String,
    peer_key: sign::PublicKey,
    password: Mutex<Option<Zeroizing<String>>>,
    timeout: Duration,
    started: AtomicBool,
    cancelled: AtomicBool,
    socket: Mutex<Option<TcpStream>>,
}

pub type ControllerSessionCallback = unsafe extern "C" fn(*const c_char, *mut c_void);

#[no_mangle]
pub extern "C" fn controller_session_create(
    request_json: *const c_char,
    password: *const c_char,
    timeout_ms: u32,
) -> *mut ControllerSession {
    if request_json.is_null() || password.is_null() || !(100..=30_000).contains(&timeout_ms) {
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
    let Ok(endpoint) = request.endpoint.parse::<SocketAddr>() else {
        return ptr::null_mut();
    };
    if endpoint.port() == 0
        || !(6..=20).contains(&request.peer_id.len())
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
        peer_id: request.peer_id,
        peer_key,
        password: Mutex::new(Some(Zeroizing::new(password.to_owned()))),
        timeout: Duration::from_millis(timeout_ms.into()),
        started: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        socket: Mutex::new(None),
    }))
}

struct Failure {
    state: &'static str,
    code: &'static str,
    message: &'static str,
}
impl Failure {
    const fn failed(code: &'static str, message: &'static str) -> Self {
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

fn status(task: &ControllerSession, deadline: Instant) -> Result<Duration, Failure> {
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

fn io_failure(task: &ControllerSession, deadline: Instant) -> Failure {
    status(task, deadline)
        .err()
        .unwrap_or(Failure::failed("TRANSPORT_FAILED", "Connection failed"))
}

struct Wire {
    stream: TcpStream,
    codec: BytesCodec,
    buffer: BytesMut,
}

impl Wire {
    fn new(stream: TcpStream) -> Self {
        let mut codec = BytesCodec::new();
        codec.set_max_packet_length(FRAME_LIMIT);
        Self {
            stream,
            codec,
            buffer: BytesMut::new(),
        }
    }

    fn receive(
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

    fn send(
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
        Message::parse_from_bytes(&bytes)
            .map_err(|_| Failure::failed("PROTOCOL_FAILED", "Invalid login message"))
    }
}

fn response(password: &mut Zeroizing<String>, challenge: &Hash, peer_id: &str) -> Message {
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
        disable_keyboard: option_message::BoolOption::Yes.into(),
        disable_audio: option_message::BoolOption::Yes.into(),
        disable_clipboard: option_message::BoolOption::Yes.into(),
        disable_camera: option_message::BoolOption::Yes.into(),
        enable_file_transfer: option_message::BoolOption::No.into(),
        block_input: option_message::BoolOption::No.into(),
        privacy_mode: option_message::BoolOption::No.into(),
        ..Default::default()
    };
    let mut message = Message::new();
    message.set_login_request(LoginRequest {
        username: peer_id.to_owned(),
        password: digest,
        my_id: "123456789".to_owned(),
        my_name: "Harmony Controller".to_owned(),
        option: MessageField::some(option),
        ..Default::default()
    });
    message
}

fn run(task: &ControllerSession, callback: Option<ControllerSessionCallback>, user: *mut c_void) {
    let deadline = Instant::now() + task.timeout;
    let mut verified = false;
    let result = (|| -> Result<(), Failure> {
        status(task, deadline)?;
        event(
            callback,
            user,
            "connecting",
            "CONNECTING",
            "Connecting to peer",
            false,
            false,
        );
        let connect_wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(2));
        if connect_wait.is_zero() {
            return Err(Failure::failed("TIMEOUT", "Login verification timed out"));
        }
        let stream = TcpStream::connect_timeout(&task.endpoint, connect_wait)
            .map_err(|_| io_failure(task, deadline))?;
        let clone = stream.try_clone().map_err(|_| io_failure(task, deadline))?;
        *task.socket.lock().unwrap() = Some(clone);
        status(task, deadline)?;
        let mut wire = Wire::new(stream);
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
        let mut password = task.password.lock().unwrap().take().ok_or(Failure::failed(
            "PROTOCOL_FAILED",
            "Missing login credential",
        ))?;
        let awaiting_approval = password.is_empty();
        let mut login = response(&mut password, &hash, &task.peer_id);
        let send_result = wire.send_message(task, deadline, &mut cipher, &login);
        if let Some(message::message::Union::LoginRequest(request)) = login.union.as_mut() {
            request.password.zeroize();
        }
        send_result?;
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
        loop {
            let message = wire.receive_message(task, deadline, &mut cipher)?;
            match message.union {
                Some(message::message::Union::LoginResponse(login)) => match login.union {
                    Some(login_response::Union::PeerInfo(_)) => {
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
                    if matches!(misc.union, Some(message::misc::Union::PermissionInfo(_))) => {}
                _ => {
                    return Err(Failure::failed(
                        "PROTOCOL_FAILED",
                        "Unexpected login message",
                    ))
                }
            }
        }
    })();
    task.password.lock().unwrap().take();
    *task.socket.lock().unwrap() = None;
    if let Err(error) = result {
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

#[no_mangle]
pub extern "C" fn controller_session_run(
    task: *mut ControllerSession,
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
) {
    if let Some(task) = unsafe { task.as_ref() } {
        if !task.started.swap(true, Ordering::AcqRel) {
            run(task, callback, user)
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_session_cancel(task: *mut ControllerSession) {
    if let Some(task) = unsafe { task.as_ref() } {
        task.cancelled.store(true, Ordering::Release);
        task.password.lock().unwrap().take();
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
