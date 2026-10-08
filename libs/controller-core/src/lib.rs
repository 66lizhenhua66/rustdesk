pub mod access;
#[path = "../../base/src/access_proof.rs"]
pub mod access_proof;
pub use protos::message as message_proto;
pub mod canvas;
mod input;
pub mod interaction;
pub mod meta;
mod rendezvous;
pub mod session;
pub mod upstream_crypto;
mod video_settings;

#[path = "../../hbb_common/src/bytes_codec.rs"]
mod bytes_codec;

pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

use std::{
    ffi::{c_char, c_void, CStr, CString},
    io::{self, Read},
    net::{IpAddr, SocketAddr, TcpStream},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::BytesMut;
use protobuf::Message as _;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::codec::Decoder;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Profile {
    id: String,
    name: String,
    mode: String,
    target: String,
    server: String,
    server_key: String,
    #[serde(default)]
    relay_server: String,
    peer_fingerprint: String,
    #[serde(default)]
    peer_id: String,
    #[serde(default)]
    peer_public_key: String,
}

struct ProfileError(&'static str, &'static str);

fn normalized_profile(mut p: Profile) -> Result<Profile, ProfileError> {
    if p.name.trim().is_empty() || p.name.len() > 128 || p.id.len() > 128 {
        return Err(ProfileError(
            "INVALID_PROFILE",
            "Invalid profile name or ID",
        ));
    }
    p.name = p.name.trim().to_owned();
    p.id = p.id.trim().to_owned();
    p.target = p.target.trim().to_owned();
    p.server = p.server.trim().to_owned();
    p.server_key = p.server_key.trim().to_owned();
    p.relay_server = p.relay_server.trim().to_owned();
    p.peer_fingerprint = p.peer_fingerprint.trim().to_ascii_lowercase();
    p.peer_id = p.peer_id.trim().to_owned();
    p.peer_public_key = p.peer_public_key.trim().to_owned();
    if !p.peer_id.is_empty()
        && (!(6..=20).contains(&p.peer_id.len()) || !p.peer_id.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(ProfileError("INVALID_PEER_ID", "Invalid expected peer ID"));
    }
    if !p.peer_public_key.is_empty()
        && STANDARD
            .decode(&p.peer_public_key)
            .map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(ProfileError(
            "INVALID_PEER_KEY",
            "Invalid trusted peer public key",
        ));
    }

    match p.mode.as_str() {
        "direct" => p.target = direct_address(&p.target)?.to_string(),
        "id" | "relay" => {
            if !(6..=20).contains(&p.target.len()) || !p.target.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(ProfileError("INVALID_TARGET", "Invalid device ID"));
            }
            if !p.peer_id.is_empty() && p.peer_id != p.target {
                return Err(ProfileError(
                    "PEER_ID_MISMATCH",
                    "Target differs from pinned peer ID",
                ));
            }
        }
        _ => return Err(ProfileError("INVALID_MODE", "Invalid connection mode")),
    }
    if !p.server.is_empty() {
        p.server = server_address(&p.server)?;
    }
    if !p.relay_server.is_empty() {
        p.relay_server = server_address_with_default(&p.relay_server, 21117)
            .map_err(|_| ProfileError("INVALID_RELAY_SERVER", "Invalid relay server address"))?;
    }
    if !p.server_key.is_empty()
        && STANDARD
            .decode(&p.server_key)
            .map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(ProfileError("INVALID_SERVER_KEY", "Invalid server key"));
    }
    if !p.peer_fingerprint.is_empty()
        && (p.peer_fingerprint.len() != 64
            || !p.peer_fingerprint.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ProfileError(
            "INVALID_FINGERPRINT",
            "Invalid peer fingerprint",
        ));
    }
    Ok(p)
}

fn direct_address(s: &str) -> Result<SocketAddr, ProfileError> {
    if let Ok(addr) = s.parse::<SocketAddr>() {
        if addr.port() != 0 {
            return Ok(addr);
        }
    }
    let ip_text = s
        .strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .unwrap_or(s);
    let ip = ip_text
        .parse::<IpAddr>()
        .map_err(|_| ProfileError("INVALID_TARGET", "Invalid literal IP address"))?;
    Ok(SocketAddr::new(ip, 21118))
}

fn server_address(s: &str) -> Result<String, ProfileError> {
    server_address_with_default(s, 21116)
}

fn server_address_with_default(s: &str, default_port: u16) -> Result<String, ProfileError> {
    let (host, port) = if let Some(tail) = s.strip_prefix('[') {
        let (host, remainder) = tail
            .split_once(']')
            .ok_or(ProfileError("INVALID_SERVER", "Invalid server address"))?;
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| ProfileError("INVALID_SERVER", "Invalid server address"))?;
        let port = if remainder.is_empty() {
            default_port
        } else {
            remainder
                .strip_prefix(':')
                .ok_or(ProfileError("INVALID_SERVER", "Invalid server address"))?
                .parse::<u16>()
                .map_err(|_| ProfileError("INVALID_SERVER", "Invalid server port"))?
        };
        (format!("[{host}]"), port)
    } else {
        let (host, port) = match s.rsplit_once(':') {
            Some((host, port)) => (
                host,
                port.parse::<u16>()
                    .map_err(|_| ProfileError("INVALID_SERVER", "Invalid server port"))?,
            ),
            None => (s, default_port),
        };
        if host.parse::<std::net::Ipv4Addr>().is_err() && !valid_hostname(host) {
            return Err(ProfileError("INVALID_SERVER", "Invalid server address"));
        }
        (host.to_ascii_lowercase(), port)
    };
    if port == 0 {
        return Err(ProfileError("INVALID_SERVER", "Invalid server port"));
    }
    Ok(format!("{host}:{port}"))
}

fn valid_hostname(s: &str) -> bool {
    s.len() <= 253
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

fn json_string(value: serde_json::Value) -> *mut c_char {
    CString::new(value.to_string()).map_or(ptr::null_mut(), CString::into_raw)
}

#[no_mangle]
pub extern "C" fn controller_version() -> *const c_char {
    c"0.2.0".as_ptr()
}

#[no_mangle]
pub extern "C" fn controller_validate_profile(profile_json: *const c_char) -> *mut c_char {
    let result = if profile_json.is_null() {
        Err(ProfileError("INVALID_JSON", "Invalid profile JSON"))
    } else {
        // SAFETY: The caller promises a valid NUL-terminated C string.
        unsafe { CStr::from_ptr(profile_json) }
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str::<Profile>(s).ok())
            .ok_or(ProfileError("INVALID_JSON", "Invalid profile JSON"))
            .and_then(normalized_profile)
    };
    match result {
        Ok(profile) => {
            let authentication_available = !profile.peer_id.is_empty()
                && !profile.peer_public_key.is_empty()
                && (profile.mode == "direct"
                    || (!profile.server.is_empty()
                        && !profile.server_key.is_empty()
                        && !profile.relay_server.is_empty()));
            json_string(json!({"ok":true,"profile":profile,"readyForSession":false,
                "authenticationAvailable":authentication_available,"reason":"REMOTE_SESSION_NOT_IMPLEMENTED"}))
        }
        Err(ProfileError(code, message)) => {
            json_string(json!({"ok":false,"code":code,"message":message}))
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_free_string(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: value was returned by controller_validate_profile exactly once.
        unsafe {
            drop(CString::from_raw(value));
        }
    }
}

pub type ControllerProbeCallback = unsafe extern "C" fn(*const c_char, *mut c_void);

pub struct ControllerProbe {
    endpoint: SocketAddr,
    timeout: Duration,
    cancelled: AtomicBool,
    started: AtomicBool,
}

#[no_mangle]
pub extern "C" fn controller_probe_create(
    endpoint: *const c_char,
    timeout_ms: u32,
) -> *mut ControllerProbe {
    if endpoint.is_null() || !(100..=2000).contains(&timeout_ms) {
        return ptr::null_mut();
    }
    // SAFETY: The caller promises a valid NUL-terminated C string.
    let addr = unsafe { CStr::from_ptr(endpoint) }
        .to_str()
        .ok()
        .and_then(|s| s.parse::<SocketAddr>().ok());
    match addr.filter(|a| a.port() != 0) {
        Some(endpoint) => Box::into_raw(Box::new(ControllerProbe {
            endpoint,
            timeout: Duration::from_millis(timeout_ms.into()),
            cancelled: AtomicBool::new(false),
            started: AtomicBool::new(false),
        })),
        None => ptr::null_mut(),
    }
}

fn emit(
    callback: Option<ControllerProbeCallback>,
    user: *mut c_void,
    state: &str,
    code: &str,
    message: &str,
) {
    if let Some(callback) = callback {
        let value = json!({"state":state,"code":code,"message":message,"verified":false,"authenticated":false,"authorized":false});
        if let Ok(value) = CString::new(value.to_string()) {
            // SAFETY: callback and user are supplied by caller; value lives for this call.
            unsafe {
                callback(value.as_ptr(), user);
            }
        }
    }
}

fn run_probe(task: &ControllerProbe, callback: Option<ControllerProbeCallback>, user: *mut c_void) {
    if task.cancelled.load(Ordering::Acquire) {
        emit(
            callback,
            user,
            "cancelled",
            "CANCELLED",
            "Network preflight cancelled",
        );
        return;
    }
    emit(
        callback,
        user,
        "connecting",
        "CONNECTING",
        "Connecting to endpoint",
    );
    let deadline = Instant::now() + task.timeout;
    let mut stream = match TcpStream::connect_timeout(&task.endpoint, task.timeout) {
        Ok(stream) => stream,
        Err(_) if task.cancelled.load(Ordering::Acquire) => {
            emit(
                callback,
                user,
                "cancelled",
                "CANCELLED",
                "Network preflight cancelled",
            );
            return;
        }
        Err(_) => {
            emit(
                callback,
                user,
                "failed",
                "TRANSPORT_FAILED",
                "TCP connection failed",
            );
            return;
        }
    };
    if task.cancelled.load(Ordering::Acquire) {
        emit(
            callback,
            user,
            "cancelled",
            "CANCELLED",
            "Network preflight cancelled",
        );
        return;
    }
    emit(
        callback,
        user,
        "transport_open",
        "TRANSPORT_OPEN",
        "TCP connection opened",
    );
    let mut codec = bytes_codec::BytesCodec::new();
    codec.set_max_packet_length(64 * 1024);
    let mut buffer = BytesMut::new();
    let mut chunk = [0u8; 4096];
    while Instant::now() < deadline {
        if task.cancelled.load(Ordering::Acquire) {
            emit(
                callback,
                user,
                "cancelled",
                "CANCELLED",
                "Network preflight cancelled",
            );
            return;
        }
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(40));
        if wait.is_zero() {
            break;
        }
        if stream.set_read_timeout(Some(wait)).is_err() {
            emit(
                callback,
                user,
                "failed",
                "TRANSPORT_FAILED",
                "TCP read setup failed",
            );
            return;
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                match codec.decode(&mut buffer) {
                    Ok(Some(frame)) => {
                        let kind =
                            match protos::message::Message::parse_from_bytes(&frame) {
                                Ok(message) => match message.union {
                                    Some(protos::message::message::Union::SignedId(id))
                                        if !id.id.is_empty() =>
                                    {
                                        Some("signed_id")
                                    }
                                    Some(protos::message::message::Union::Hash(hash))
                                        if !hash.challenge.is_empty() =>
                                    {
                                        Some("hash")
                                    }
                                    Some(protos::message::message::Union::LoginResponse(
                                        response,
                                    )) if response.union.is_some() => Some("login_response"),
                                    _ => None,
                                },
                                Err(_) => None,
                            };
                        if let Some(kind) = kind {
                            emit(callback, user, "protocol_message", "PROTOCOL_MESSAGE", kind);
                        }
                        emit(
                            callback,
                            user,
                            "blocked",
                            "AUTHENTICATION_REQUIRED",
                            "Network preflight does not authenticate peers",
                        );
                        return;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        emit(
                            callback,
                            user,
                            "failed",
                            "FRAME_TOO_LARGE",
                            "Invalid or oversized protocol frame",
                        );
                        return;
                    }
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::TimedOut || e.kind() == io::ErrorKind::WouldBlock => {
            }
            Err(_) => break,
        }
    }
    if task.cancelled.load(Ordering::Acquire) {
        emit(
            callback,
            user,
            "cancelled",
            "CANCELLED",
            "Network preflight cancelled",
        );
    } else {
        emit(
            callback,
            user,
            "blocked",
            "AUTHENTICATION_REQUIRED",
            "Network preflight does not authenticate peers",
        );
    }
}

#[no_mangle]
pub extern "C" fn controller_probe_run(
    task: *mut ControllerProbe,
    callback: Option<ControllerProbeCallback>,
    user: *mut c_void,
) {
    if let Some(task) = unsafe { task.as_ref() } {
        if !task.started.swap(true, Ordering::AcqRel) {
            run_probe(task, callback, user);
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_probe_cancel(task: *mut ControllerProbe) {
    if let Some(task) = unsafe { task.as_ref() } {
        task.cancelled.store(true, Ordering::Release);
    }
}

#[no_mangle]
pub extern "C" fn controller_probe_destroy(task: *mut ControllerProbe) {
    if !task.is_null() {
        // SAFETY: caller must wait for run to return and destroy each task once.
        unsafe {
            drop(Box::from_raw(task));
        }
    }
}
