use std::{
    ffi::{c_char, c_void, CStr, CString},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    ptr,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use bytes::BytesMut;
use protobuf::Message as _;
use remote_controller_core::session::{
    approval_code, controller_connection_create, controller_session_cancel,
    controller_session_create, controller_session_destroy, controller_session_run,
    controller_session_run_video, controller_session_send_input_v1,
    controller_session_send_pointer, controller_session_send_text,
    controller_session_set_input_enabled_v1,
};
use remote_controller_core::{
    protos::{
        message::{
            self, login_response, option_message, permission_info, ChatMessage, DisplayInfo,
            EncodedVideoFrame, EncodedVideoFrames, Hash, LoginResponse, Message, Misc, PeerInfo,
            PermissionInfo, SignedId, VideoFrame,
        },
        rendezvous::IdPk,
    },
    upstream_crypto::{Encrypt, KxTranscript},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sodiumoxide::crypto::{box_, sign};

#[path = "support/video_settings.rs"]
mod video_settings;
#[path = "support/video_resolution.rs"]
mod video_resolution;

unsafe extern "C" fn collect(event: *const c_char, user: *mut c_void) {
    let events = &mut *(user as *mut Vec<Value>);
    events.push(serde_json::from_str(CStr::from_ptr(event).to_str().unwrap()).unwrap());
}

fn request(endpoint: &str) -> CString {
    CString::new(json!({"endpoint":endpoint,"peerId":"123456789","peerPublicKey":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}).to_string()).unwrap()
}

#[test]
fn demo_pairing_code_is_stable() {
    let digest = Sha256::digest(b"nonce");
    let value = u32::from_be_bytes(digest[..4].try_into().unwrap()) % 1_000_000;
    assert_eq!(approval_code("nonce"), format!("{value:06}"));
}

#[test]
fn demo_api_validates_handshake_and_input_before_connecting() {
    let password = CString::new("").unwrap();
    let req = request("127.0.0.1:1");
    assert!(controller_connection_create(req.as_ptr(), password.as_ptr(), 99).is_null());
    assert!(controller_connection_create(req.as_ptr(), password.as_ptr(), 60_001).is_null());
    let task = controller_connection_create(req.as_ptr(), password.as_ptr(), 100);
    assert!(!task.is_null());
    assert_eq!(controller_session_send_pointer(task, 800, 0), 3);
    assert_eq!(controller_session_send_pointer(task, 0, 450), 3);
    assert_eq!(controller_session_send_pointer(task, 0, 0), 1);
    assert_eq!(
        controller_session_send_text(task, CString::new("").unwrap().as_ptr()),
        3
    );
    assert_eq!(
        controller_session_send_text(task, CString::new("text").unwrap().as_ptr()),
        1
    );
    controller_session_destroy(task);
}

#[test]
fn video_settings_request_requires_a_valid_pair_and_control_mode() {
    let base: Value = serde_json::from_str(request("127.0.0.1:1").to_str().unwrap()).unwrap();
    for (quality, fps, mode, valid) in [
        (Some(json!("balanced")), Some(json!(15)), "secure_control", true),
        (Some(json!("low")), Some(json!(10)), "secure_control", true),
        (Some(json!("high")), Some(json!(30)), "secure_control", true),
        (Some(json!("balanced")), None, "secure_control", false),
        (None, Some(json!(15)), "secure_control", false),
        (Some(json!("ultra")), Some(json!(15)), "secure_control", false),
        (Some(json!("high")), Some(json!(60)), "secure_control", false),
        (Some(Value::Null), Some(Value::Null), "secure_control", false),
        (Some(json!("balanced")), Some(json!(15)), "secure_video", false),
    ] {
        let mut value = base.clone();
        value["expectedPeer"] = json!(mode);
        if let Some(quality) = quality { value["videoQuality"] = quality; }
        if let Some(fps) = fps { value["videoFps"] = fps; }
        let task = controller_connection_create(CString::new(value.to_string()).unwrap().as_ptr(), c"".as_ptr(), 100);
        assert_eq!(!task.is_null(), valid, "{value}");
        controller_session_destroy(task);
    }
}

#[test]
fn resolution_settings_login_requires_complete_preserve_choice() {
    let base: Value = serde_json::from_str(request("127.0.0.1:1").to_str().unwrap()).unwrap();
    for (resolution, valid) in [
        (json!({"videoResolutionMode":"preserve","videoResolutionWidth":0,"videoResolutionHeight":0}), true),
        (json!({"videoResolutionMode":"preserve","videoResolutionWidth":1280,"videoResolutionHeight":720}), true),
        (json!({"videoResolutionMode":"sync","videoResolutionWidth":1280,"videoResolutionHeight":720}), false),
        (json!({"videoResolutionMode":"preserve","videoResolutionWidth":1280}), false),
        (json!({"videoResolutionMode":"preserve","videoResolutionWidth":0,"videoResolutionHeight":720}), false),
        (json!({"videoResolutionMode":"preserve","videoResolutionWidth":1600,"videoResolutionHeight":900}), false),
        (json!({"videoResolutionMode":null,"videoResolutionWidth":0,"videoResolutionHeight":0}), false),
    ] {
        let mut value = base.clone();
        value["expectedPeer"] = json!("secure_control");
        value["videoQuality"] = json!("balanced");
        value["videoFps"] = json!(15);
        value.as_object_mut().unwrap().extend(resolution.as_object().unwrap().clone());
        let task = controller_connection_create(CString::new(value.to_string()).unwrap().as_ptr(), c"".as_ptr(), 100);
        assert_eq!(!task.is_null(), valid, "{value}");
        controller_session_destroy(task);
    }
}

#[test]
fn secure_control_input_api_rejects_bad_commands_and_disconnected_sessions() {
    sodiumoxide::init().unwrap();
    let request = CString::new(
        json!({
            "endpoint":"127.0.0.1:1", "peerId":"123456789",
            "peerPublicKey":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "expectedPeer":"secure_control"
        })
        .to_string(),
    )
    .unwrap();
    let password = CString::new("").unwrap();
    let task = controller_connection_create(request.as_ptr(), password.as_ptr(), 100);
    assert!(!task.is_null());
    assert_eq!(
        controller_session_send_input_v1(
            task,
            CString::new(r#"{"kind":"move","x":65536,"y":0}"#)
                .unwrap()
                .as_ptr()
        ),
        3
    );
    assert_eq!(
        controller_session_send_input_v1(
            task,
            CString::new(r#"{"kind":"move","x":1,"y":2}"#)
                .unwrap()
                .as_ptr()
        ),
        1
    );
    assert_eq!(controller_session_set_input_enabled_v1(task, 1), 1);
    assert_eq!(controller_session_set_input_enabled_v1(task, 2), 3);
    controller_session_destroy(task);
}

fn trusted_request(endpoint: &str, key: &sign::PublicKey, peer_id: &str) -> CString {
    use base64::Engine as _;
    CString::new(json!({"endpoint":endpoint,"peerId":peer_id,"peerPublicKey":base64::engine::general_purpose::STANDARD.encode(key.0),"peerFingerprint":format!("{:x}", Sha256::digest(key.0)),"minimumKxVersion":1}).to_string()).unwrap()
}

fn send_frame(stream: &mut TcpStream, data: &[u8]) {
    assert!(data.len() < 0x4000);
    let header = ((data.len() << 2) as u16 | 1).to_le_bytes();
    stream.write_all(&header).unwrap();
    stream.write_all(data).unwrap();
}

fn recv_frame(stream: &mut TcpStream) -> Vec<u8> {
    let mut first = [0u8; 1];
    stream.read_exact(&mut first).unwrap();
    let head_len = ((first[0] & 3) + 1) as usize;
    let mut header = [0u8; 4];
    header[0] = first[0];
    stream.read_exact(&mut header[1..head_len]).unwrap();
    let size = (u32::from_le_bytes(header) >> 2) as usize;
    let mut payload = vec![0u8; size];
    stream.read_exact(&mut payload).unwrap();
    payload
}

fn send_message(stream: &mut TcpStream, message: &Message) {
    send_frame(stream, &message.write_to_bytes().unwrap())
}

fn signed_identity(
    peer_id: &str,
    key: &sign::SecretKey,
    ephemeral: &box_::PublicKey,
    version: u32,
) -> Message {
    let idpk = IdPk {
        id: peer_id.to_owned(),
        pk: ephemeral.0.to_vec(),
        kx_version: version,
        ..Default::default()
    };
    let mut message = Message::new();
    message.set_signed_id(SignedId {
        id: sign::sign(&idpk.write_to_bytes().unwrap(), key),
        ..Default::default()
    });
    message
}

fn negotiated(
    stream: &mut TcpStream,
    key: &sign::SecretKey,
    peer_id: &str,
    version: u32,
) -> Encrypt {
    let (ephemeral, private) = box_::gen_keypair();
    send_message(stream, &signed_identity(peer_id, key, &ephemeral, version));
    let response = Message::parse_from_bytes(&recv_frame(stream)).unwrap();
    let Some(message::message::Union::PublicKey(public)) = response.union else {
        panic!("expected public key")
    };
    assert_eq!(public.kx_version, 1);
    let shared =
        Encrypt::decode(&public.symmetric_value, &public.asymmetric_value, &private).unwrap();
    Encrypt::new_split(
        shared,
        false,
        &KxTranscript {
            initiator_pk: &public.asymmetric_value,
            responder_pk: &ephemeral.0,
            advertised: version,
            picked: 1,
        },
    )
    .unwrap()
}

fn send_encrypted(stream: &mut TcpStream, cipher: &mut Encrypt, message: &Message) {
    send_frame(stream, &cipher.enc(&message.write_to_bytes().unwrap()));
}

fn receive_encrypted(stream: &mut TcpStream, cipher: &mut Encrypt) -> Message {
    let raw = recv_frame(stream);
    let mut encrypted = BytesMut::from(raw.as_slice());
    cipher.dec(&mut encrypted).unwrap();
    Message::parse_from_bytes(&encrypted).unwrap()
}

fn execute<F>(server_fn: F) -> Vec<Value>
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    execute_with_password("correct horse", server_fn)
}

fn execute_with_password<F>(password: &str, server_fn: F) -> Vec<Value>
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server_fn(stream, private);
    });
    let password = CString::new(password).unwrap();
    let task = controller_session_create(
        trusted_request(&endpoint, &public, "123456789").as_ptr(),
        password.as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
    controller_session_destroy(task);
    server.join().unwrap();
    events
}

fn execute_demo<F>(server_fn: F) -> Vec<Value>
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    execute_persistent(None, server_fn)
}

fn execute_persistent<F>(expected: Option<&str>, server_fn: F) -> Vec<Value>
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server_fn(stream, private);
    });
    let mut request: Value = serde_json::from_str(
        trusted_request(&endpoint, &public, "123456789")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    if let Some(mode) = expected {
        request["expectedPeer"] = json!(mode);
    }
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    if expected == Some("secure_host") {
        let mut context = DemoCollector {
            events,
            task,
            sends: Vec::new(),
        };
        controller_session_run(
            task,
            Some(collect_demo),
            &mut context as *mut _ as *mut c_void,
        );
        assert!(context.sends.iter().all(|code| *code == 2));
        events = context.events;
    } else {
        controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
    }
    controller_session_destroy(task);
    server.join().unwrap();
    events
}

fn demo_login(stream: &mut TcpStream, cipher: &mut Encrypt, additions: &str) {
    let mut challenge = Message::new();
    challenge.set_hash(Hash {
        salt: "salt".into(),
        challenge: "nonce".into(),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &challenge);
    let login = receive_encrypted(stream, cipher);
    let Some(message::message::Union::LoginRequest(login)) = login.union else {
        panic!("expected login")
    };
    assert!(login.password.is_empty());
    assert_eq!(
        login.option.unwrap().disable_keyboard.enum_value().unwrap(),
        option_message::BoolOption::No
    );
    let mut response = Message::new();
    response.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo {
            platform_additions: additions.into(),
            ..Default::default()
        })),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &response);
}

#[test]
fn persistent_connection_rejects_plain_peer_info() {
    let events = execute_demo(|mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        demo_login(&mut stream, &mut cipher, "");
        let close = receive_encrypted(&mut stream, &mut cipher);
        assert!(
            matches!(close.union, Some(message::message::Union::Misc(misc)) if matches!(misc.union, Some(message::misc::Union::CloseReason(_))))
        );
    });
    assert!(events.iter().any(|e| e["code"] == "UNSUPPORTED_PEER"));
    assert!(events
        .iter()
        .all(|e| e["authenticated"] == false && e["authorized"] == false));
    assert!(events
        .iter()
        .any(|e| e["state"] == "awaiting_approval"
            && e["confirmationCode"] == approval_code("nonce")));
}

#[test]
fn persistent_connection_accepts_official_secure_host_as_read_only() {
    let events = execute_persistent(Some("secure_host"), |mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        demo_login(
            &mut stream,
            &mut cipher,
            r#"{"ord_secure_host":1,"media":false,"input_scope":"none"}"#,
        );
        send_permission(&mut stream, &mut cipher, false);
        let mut close = Message::new();
        let mut misc = Misc::new();
        misc.set_close_reason("done".into());
        close.set_misc(misc);
        send_encrypted(&mut stream, &mut cipher, &close);
    });
    assert!(events.iter().any(|e| {
        e["state"] == "connected"
            && e["message"] == "Formal secure entry established; read-only"
            && e["authorized"] == false
    }));
    assert!(events.iter().all(|e| e["authorized"] == false));
}

#[test]
fn selected_persistent_mode_cannot_be_changed_by_peer() {
    for (expected, capabilities) in [
        (
            None,
            r#"{"ord_secure_host":1,"media":false,"input_scope":"none"}"#,
        ),
        (
            Some("secure_host"),
            r#"{"ord_demo":1,"input_scope":"demo_window","width":800,"height":450}"#,
        ),
        (
            Some("secure_host"),
            r#"{"ord_secure_host":1,"ord_demo":1,"media":false,"input_scope":"none"}"#,
        ),
    ] {
        let events = execute_persistent(expected, move |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            demo_login(&mut stream, &mut cipher, capabilities);
            let close = receive_encrypted(&mut stream, &mut cipher);
            assert!(close.has_misc());
        });
        assert!(events.iter().any(|e| e["code"] == "UNSUPPORTED_PEER"));
        assert!(events
            .iter()
            .all(|e| e["authenticated"] == false && e["authorized"] == false));
    }
}

#[test]
fn secure_host_rejects_any_permission_grant() {
    for permission in [
        permission_info::Permission::Keyboard,
        permission_info::Permission::File,
        permission_info::Permission::Audio,
    ] {
        let events = execute_persistent(Some("secure_host"), move |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            demo_login(
                &mut stream,
                &mut cipher,
                r#"{"ord_secure_host":1,"media":false,"input_scope":"none"}"#,
            );
            let mut msg = Message::new();
            let mut misc = Misc::new();
            misc.set_permission_info(PermissionInfo {
                permission: permission.into(),
                enabled: true,
                ..Default::default()
            });
            msg.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &msg);
        });
        assert!(events.iter().any(|e| e["code"] == "UNEXPECTED_PERMISSION"));
        assert!(events.iter().all(|e| e["authorized"] == false));
    }
}

#[test]
fn secure_host_rejects_permission_grant_before_approval() {
    let events = execute_persistent(Some("secure_host"), |mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".into(),
            challenge: "nonce".into(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        assert!(receive_encrypted(&mut stream, &mut cipher).has_login_request());
        send_permission(&mut stream, &mut cipher, true);
    });
    assert!(events
        .iter()
        .any(|event| event["code"] == "UNEXPECTED_PERMISSION"));
    assert!(events
        .iter()
        .all(|event| event["authenticated"] == false && event["authorized"] == false));
}

fn send_permission(stream: &mut TcpStream, cipher: &mut Encrypt, enabled: bool) {
    let mut misc = Misc::new();
    misc.set_permission_info(PermissionInfo {
        permission: permission_info::Permission::Keyboard.into(),
        enabled,
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_misc(misc);
    send_encrypted(stream, cipher, &message);
}

#[test]
fn video_mode_without_sink_refuses_before_connect() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let (public, _) = sign::gen_keypair();
    let mut request: Value = serde_json::from_str(
        trusted_request(
            &listener.local_addr().unwrap().to_string(),
            &public,
            "123456789",
        )
        .to_str()
        .unwrap(),
    )
    .unwrap();
    request["expectedPeer"] = json!("secure_video");
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1000,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
    assert!(events.iter().any(|e| e["code"] == "VIDEO_SINK_REQUIRED"));
    assert!(listener.accept().is_err());
    controller_session_destroy(task);
}

#[test]
fn video_entry_rejects_non_video_request_before_connect() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let (public, _) = sign::gen_keypair();
    let task = controller_connection_create(
        trusted_request(
            &listener.local_addr().unwrap().to_string(),
            &public,
            "123456789",
        )
        .as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1000,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run_video(
        task,
        Some(collect),
        Some(discard_video),
        &mut events as *mut _ as *mut c_void,
    );
    assert!(events
        .iter()
        .any(|event| event["code"] == "VIDEO_MODE_REQUIRED"));
    assert!(listener.accept().is_err());
    controller_session_destroy(task);
}

#[test]
fn video_mode_rejects_frame_before_approval() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut cipher = negotiated(&mut stream, &private, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".into(),
            challenge: "nonce".into(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        let login = receive_encrypted(&mut stream, &mut cipher);
        let Some(message::message::Union::LoginRequest(login)) = login.union else {
            panic!("expected login")
        };
        let option = login.option.unwrap();
        assert_eq!(
            option.disable_keyboard.enum_value().unwrap(),
            option_message::BoolOption::Yes
        );
        let decoding = option.supported_decoding.unwrap();
        assert_eq!(decoding.ability_vp8, 1);
        assert_eq!(
            decoding.prefer.enum_value().unwrap(),
            message::supported_decoding::PreferCodec::VP8
        );
        let mut video = Message::new();
        video.set_video_frame(Default::default());
        send_encrypted(&mut stream, &mut cipher, &video);
    });
    let mut request: Value = serde_json::from_str(
        trusted_request(&endpoint, &public, "123456789")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    request["expectedPeer"] = json!("secure_video");
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run_video(
        task,
        Some(collect),
        Some(discard_video),
        &mut events as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    assert!(events.iter().any(|e| e["code"] == "UNEXPECTED_VIDEO"));
    assert!(events.iter().all(|e| e["authorized"] == false));
}

#[test]
fn video_mode_rejects_permission_grant_before_approval() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut cipher = negotiated(&mut stream, &private, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".into(),
            challenge: "nonce".into(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        assert!(receive_encrypted(&mut stream, &mut cipher).has_login_request());
        send_permission(&mut stream, &mut cipher, true);
    });
    let mut request: Value = serde_json::from_str(
        trusted_request(&endpoint, &public, "123456789")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    request["expectedPeer"] = json!("secure_video");
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run_video(
        task,
        Some(collect),
        Some(discard_video),
        &mut events as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    assert!(events
        .iter()
        .any(|event| event["code"] == "UNEXPECTED_PERMISSION"));
}

unsafe extern "C" fn discard_video(
    _: *const u8,
    _: u32,
    _: u32,
    _: u32,
    _: i64,
    _: u8,
    _: *mut c_void,
) {
}

fn video_login(
    stream: &mut TcpStream,
    cipher: &mut Encrypt,
    additions: &str,
    width: i32,
    height: i32,
) {
    let mut challenge = Message::new();
    challenge.set_hash(Hash {
        salt: "salt".into(),
        challenge: "nonce".into(),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &challenge);
    assert!(receive_encrypted(stream, cipher).has_login_request());
    let mut response = Message::new();
    response.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo {
            platform_additions: additions.into(),
            displays: vec![DisplayInfo {
                width,
                height,
                ..Default::default()
            }],
            current_display: 0,
            ..Default::default()
        })),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &response);
}

fn video_message(frames: Vec<EncodedVideoFrame>, display: i32) -> Message {
    let mut message = Message::new();
    message.set_video_frame(VideoFrame {
        union: Some(message::video_frame::Union::Vp8s(EncodedVideoFrames {
            frames,
            ..Default::default()
        })),
        display,
        ..Default::default()
    });
    message
}

struct VideoCollector {
    events: Vec<Value>,
    frames: Vec<(Vec<u8>, u32, u32, i64, u8)>,
    task: *mut remote_controller_core::session::ControllerSession,
    cancel_after_first: bool,
}

unsafe extern "C" fn collect_video_event(event: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut VideoCollector);
    context
        .events
        .push(serde_json::from_str(CStr::from_ptr(event).to_str().unwrap()).unwrap());
}

unsafe extern "C" fn collect_video(
    data: *const u8,
    length: u32,
    width: u32,
    height: u32,
    pts: i64,
    key_frame: u8,
    user: *mut c_void,
) {
    let context = &mut *(user as *mut VideoCollector);
    context.frames.push((
        std::slice::from_raw_parts(data, length as usize).to_vec(),
        width,
        height,
        pts,
        key_frame,
    ));
    if context.cancel_after_first {
        controller_session_cancel(context.task);
    }
}

fn execute_video<F>(server_fn: F, cancel_after_first: bool) -> VideoCollector
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server_fn(stream, private);
    });
    let mut request: Value = serde_json::from_str(
        trusted_request(&endpoint, &public, "123456789")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    request["expectedPeer"] = json!("secure_video");
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut context = VideoCollector {
        events: Vec::new(),
        frames: Vec::new(),
        task,
        cancel_after_first,
    };
    controller_session_run_video(
        task,
        Some(collect_video_event),
        Some(collect_video),
        &mut context as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    context
}

const VIDEO_ADDITIONS: &str =
    r#"{"ord_secure_host":1,"media":true,"video_codec":"vp8","input_scope":"none"}"#;
const CONTROL_ADDITIONS: &str = r#"{"ord_secure_host":1,"media":true,"video_codec":"vp8","input_scope":"windows_primary","input_version":2}"#;

fn control_login(stream: &mut TcpStream, cipher: &mut Encrypt, additions: &str) {
    let mut challenge = Message::new();
    challenge.set_hash(Hash {
        salt: "salt".into(),
        challenge: "nonce".into(),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &challenge);
    let login = receive_encrypted(stream, cipher);
    let Some(message::message::Union::LoginRequest(login)) = login.union else {
        panic!("expected login request")
    };
    assert_eq!(login.ord_input_version, 2);
    assert_eq!(
        login.option.unwrap().disable_keyboard.enum_value(),
        Ok(option_message::BoolOption::No)
    );
    let mut response = Message::new();
    response.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo {
            platform_additions: additions.into(),
            displays: vec![DisplayInfo {
                width: 640,
                height: 360,
                online: true,
                ..Default::default()
            }],
            current_display: 0,
            ..Default::default()
        })),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &response);
}

fn send_initial_control_state(stream: &mut TcpStream, cipher: &mut Encrypt) {
    let mut state = Message::new();
    state.set_ord_input_state(message::OrdInputState {
        version: 1,
        supported: true,
        enabled: false,
        request_id: 0,
        ..Default::default()
    });
    send_encrypted(stream, cipher, &state);
}

fn grant_control(stream: &mut TcpStream, cipher: &mut Encrypt, token: Vec<u8>) -> u64 {
    send_initial_control_state(stream, cipher);
    let request = receive_encrypted(stream, cipher);
    let Some(message::message::Union::OrdInputRequest(request)) = request.union else {
        panic!("expected controller input request")
    };
    assert_eq!(request.version, 1);
    assert!(request.enabled);
    assert_eq!(request.scope, "windows_primary");
    assert!(request.request_id > 0);
    let mut state = Message::new();
    state.set_ord_input_state(message::OrdInputState {
        version: 1,
        supported: true,
        enabled: true,
        grant_token: token,
        request_id: request.request_id,
        ..Default::default()
    });
    send_encrypted(stream, cipher, &state);
    request.request_id
}

struct ControlCollector {
    events: Vec<Value>,
    sends: Vec<i32>,
    legacy_sends: Vec<i32>,
    task: *mut remote_controller_core::session::ControllerSession,
    submit_input: bool,
    request_input: bool,
    requested: bool,
    request_codes: Vec<i32>,
    off_on_grant: bool,
    off_sent: bool,
    off_codes: Vec<i32>,
}

unsafe extern "C" fn collect_control_event(event: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut ControlCollector);
    let event: Value = serde_json::from_str(CStr::from_ptr(event).to_str().unwrap()).unwrap();
    if context.request_input
        && !context.requested
        && event["state"] == "input_state"
        && event["inputSupported"] == true
    {
        context.requested = true;
        context
            .request_codes
            .push(controller_session_set_input_enabled_v1(context.task, 1));
    }
    if context.off_on_grant
        && !context.off_sent
        && event["state"] == "input_state"
        && event["authorized"] == true
    {
        context.off_sent = true;
        context
            .off_codes
            .push(controller_session_set_input_enabled_v1(context.task, 0));
    }
    if context.submit_input && event["state"] == "input_state" {
        context.sends.push(controller_session_send_input_v1(
            context.task,
            CString::new(r#"{"kind":"move","x":32768,"y":65535}"#)
                .unwrap()
                .as_ptr(),
        ));
        if event["authorized"] == true {
            context
                .legacy_sends
                .push(controller_session_send_pointer(context.task, 1, 2));
            context.legacy_sends.push(controller_session_send_text(
                context.task,
                CString::new("legacy").unwrap().as_ptr(),
            ));
        }
    }
    context.events.push(event);
}

fn execute_control<F>(server_fn: F, submit_input: bool, request_input: bool) -> ControlCollector
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    execute_control_with_off(server_fn, submit_input, request_input, false)
}

fn execute_control_with_off<F>(
    server_fn: F,
    submit_input: bool,
    request_input: bool,
    off_on_grant: bool,
) -> ControlCollector
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let started = Instant::now();
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(2) =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("control peer did not connect: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server_fn(stream, private);
    });
    let mut request: Value = serde_json::from_str(
        trusted_request(&endpoint, &public, "123456789")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    request["expectedPeer"] = json!("secure_control");
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut context = ControlCollector {
        events: Vec::new(),
        sends: Vec::new(),
        legacy_sends: Vec::new(),
        task,
        submit_input,
        request_input,
        requested: false,
        request_codes: Vec::new(),
        off_on_grant,
        off_sent: false,
        off_codes: Vec::new(),
    };
    controller_session_run_video(
        task,
        Some(collect_control_event),
        Some(discard_video),
        &mut context as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    context
}

#[test]
fn control_mode_rejects_old_read_only_peer_metadata() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, VIDEO_ADDITIONS);
            assert!(receive_encrypted(&mut stream, &mut cipher).has_misc());
        },
        false,
        false,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "UNSUPPORTED_PEER"));
    assert!(result
        .events
        .iter()
        .all(|event| event["authorized"] == false));
}

#[test]
fn control_mode_requires_real_state_and_revokes_input() {
    let token = vec![7; 16];
    let server_token = token.clone();
    let result = execute_control(
        move |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            grant_control(&mut stream, &mut cipher, server_token.clone());
            send_permission(&mut stream, &mut cipher, true);
            let sent = receive_encrypted(&mut stream, &mut cipher);
            let Some(message::message::Union::OrdInputEvent(event)) = sent.union else {
                panic!("expected authorized input event")
            };
            assert_eq!(event.version, 1);
            assert_eq!(event.grant_token, server_token);
            assert!(
                matches!(event.command, Some(message::ord_input_event::Command::Move(m))
            if m.x == 32768 && m.y == 65535)
            );
            let mut revoke = Message::new();
            revoke.set_ord_input_state(message::OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                ..Default::default()
            });
            send_encrypted(&mut stream, &mut cipher, &revoke);
            send_permission(&mut stream, &mut cipher, false);
            let mut close = Message::new();
            let mut misc = Misc::new();
            misc.set_close_reason("done".into());
            close.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &close);
        },
        true,
        true,
    );
    assert_eq!(result.sends, [2, 0, 2]);
    assert_eq!(result.legacy_sends, [1, 1]);
    assert!(result
        .events
        .iter()
        .any(|event| event["state"] == "input_state"
            && event["inputSupported"] == true
            && event["authorized"] == true));
    assert!(result
        .events
        .iter()
        .any(|event| event["state"] == "input_state"
            && event["inputSupported"] == true
            && event["authorized"] == false));
}

#[test]
fn control_heartbeat_stops_after_revoke() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            grant_control(&mut stream, &mut cipher, vec![9; 16]);
            let heartbeat = receive_encrypted(&mut stream, &mut cipher);
            let Some(message::message::Union::OrdInputEvent(event)) = heartbeat.union else {
                panic!("expected input heartbeat")
            };
            assert_eq!(event.grant_token, vec![9; 16]);
            assert!(matches!(
                event.command,
                Some(message::ord_input_event::Command::KeepAlive(_))
            ));
            let mut revoke = Message::new();
            revoke.set_ord_input_state(message::OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                grant_token: Vec::new(),
                ..Default::default()
            });
            send_encrypted(&mut stream, &mut cipher, &revoke);
            stream
                .set_read_timeout(Some(Duration::from_millis(1500)))
                .unwrap();
            let mut byte = [0];
            assert!(
                matches!(stream.read(&mut byte), Err(error) if error.kind() == std::io::ErrorKind::TimedOut
            || error.kind() == std::io::ErrorKind::WouldBlock)
            );
            let mut close = Message::new();
            let mut misc = Misc::new();
            misc.set_close_reason("done".into());
            close.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &close);
        },
        false,
        true,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["state"] == "input_state" && event["authorized"] == false));
}

#[test]
fn control_downlink_stall_stops_heartbeats_without_eof() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            grant_control(&mut stream, &mut cipher, vec![8; 16]);
            let first = receive_encrypted(&mut stream, &mut cipher);
            assert!(
                matches!(first.union, Some(message::message::Union::OrdInputEvent(event))
            if matches!(event.command, Some(message::ord_input_event::Command::KeepAlive(_))))
            );
            stream
                .set_read_timeout(Some(Duration::from_secs(7)))
                .unwrap();
            let started = Instant::now();
            let mut byte = [0];
            loop {
                match stream.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) if started.elapsed() < Duration::from_secs(7) => {}
                    result => {
                        panic!("expected control session to close after downlink stall: {result:?}")
                    }
                }
            }
        },
        false,
        true,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "TRANSPORT_STALLED"));
}

#[test]
fn read_only_control_session_can_wait_on_a_static_desktop() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            send_initial_control_state(&mut stream, &mut cipher);
            thread::sleep(Duration::from_millis(5500));
            let mut close = Message::new();
            let mut misc = Misc::new();
            misc.set_close_reason("done".into());
            close.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &close);
        },
        false,
        false,
    );
    assert!(!result
        .events
        .iter()
        .any(|event| event["code"] == "TRANSPORT_STALLED"));
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "DISCONNECTED"));
}

#[test]
fn echoed_control_heartbeats_keep_a_static_authorized_session_live() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            grant_control(&mut stream, &mut cipher, vec![6; 16]);
            for _ in 0..6 {
                let heartbeat = receive_encrypted(&mut stream, &mut cipher);
                assert!(matches!(&heartbeat.union,
                Some(message::message::Union::OrdInputEvent(event))
                if event.grant_token == vec![6; 16]
                    && matches!(event.command, Some(message::ord_input_event::Command::KeepAlive(_)))));
                send_encrypted(&mut stream, &mut cipher, &heartbeat);
            }
            let mut close = Message::new();
            let mut misc = Misc::new();
            misc.set_close_reason("done".into());
            close.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &close);
        },
        false,
        true,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "DISCONNECTED"));
    assert!(!result
        .events
        .iter()
        .any(|event| event["code"] == "TRANSPORT_STALLED"));
}

#[test]
fn mismatched_token_heartbeat_echo_is_ignored() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            grant_control(&mut stream, &mut cipher, vec![6; 16]);
            let mut bad = Message::new();
            let mut event = message::OrdInputEvent::new();
            event.version = 1;
            event.grant_token = vec![7; 16];
            event.set_keep_alive(message::OrdInputKeepAlive::new());
            bad.set_ord_input_event(event);
            send_encrypted(&mut stream, &mut cipher, &bad);
        },
        false,
        true,
    );
    assert!(!result
        .events
        .iter()
        .any(|event| event["code"] == "INVALID_INPUT_HEARTBEAT"));
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "DISCONNECTED"));
}

#[test]
fn malformed_heartbeat_echo_fails_closed() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            grant_control(&mut stream, &mut cipher, vec![6; 16]);
            let mut bad = Message::new();
            let mut event = message::OrdInputEvent::new();
            event.version = 2;
            event.grant_token = vec![6; 16];
            event.set_keep_alive(message::OrdInputKeepAlive::new());
            bad.set_ord_input_event(event);
            send_encrypted(&mut stream, &mut cipher, &bad);
        },
        false,
        true,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "INVALID_INPUT_HEARTBEAT" && event["authorized"] == false));
}

#[test]
fn release_failure_after_revoke_remains_visible_to_controller() {
    let result = execute_control(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            send_initial_control_state(&mut stream, &mut cipher);
            let mut close = Message::new();
            let mut misc = Misc::new();
            misc.set_close_reason("Input release failed".into());
            close.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &close);
        },
        false,
        false,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "INPUT_RELEASE_FAILED" && event["authorized"] == false));
}

#[test]
fn delayed_old_heartbeat_after_local_off_keeps_video_connected() {
    let result = execute_control_with_off(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            control_login(&mut stream, &mut cipher, CONTROL_ADDITIONS);
            let on_id = grant_control(&mut stream, &mut cipher, vec![9; 16]);
            let request = receive_encrypted(&mut stream, &mut cipher);
            let Some(message::message::Union::OrdInputRequest(request)) = request.union else {
                panic!("expected input-off request")
            };
            assert!(!request.enabled);
            assert!(request.request_id > on_id);
            let mut old_echo = Message::new();
            let mut event = message::OrdInputEvent::new();
            event.version = 1;
            event.grant_token = vec![9; 16];
            event.set_keep_alive(message::OrdInputKeepAlive::new());
            old_echo.set_ord_input_event(event);
            send_encrypted(&mut stream, &mut cipher, &old_echo);
            let mut revoked = Message::new();
            revoked.set_ord_input_state(message::OrdInputState {
                version: 1,
                supported: true,
                enabled: false,
                request_id: request.request_id,
                ..Default::default()
            });
            send_encrypted(&mut stream, &mut cipher, &revoked);
            let mut close = Message::new();
            let mut misc = Misc::new();
            misc.set_close_reason("done".into());
            close.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &close);
        },
        false,
        true,
        true,
    );
    assert_eq!(result.request_codes, [0]);
    assert_eq!(result.off_codes, [0]);
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "DISCONNECTED"));
    assert!(!result
        .events
        .iter()
        .any(|event| event["code"] == "INVALID_INPUT_HEARTBEAT"));
}

#[test]
fn approved_video_delivers_binary_frames_and_keeps_input_unauthorized() {
    let body = vec![0x9a; 70_000];
    let server_body = body.clone();
    let result = execute_video(
        move |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            video_login(&mut stream, &mut cipher, VIDEO_ADDITIONS, 1280, 720);
            let message = video_message(
                vec![EncodedVideoFrame {
                    data: server_body,
                    key: true,
                    pts: 42,
                    ..Default::default()
                }],
                0,
            );
            let encrypted = cipher.enc(&message.write_to_bytes().unwrap());
            let header = (((encrypted.len() as u32) << 2) | 3).to_le_bytes();
            stream.write_all(&header).unwrap();
            stream.write_all(&encrypted).unwrap();
        },
        false,
    );
    assert_eq!(result.frames, vec![(body, 1280, 720, 42, 1)]);
    assert!(result
        .events
        .iter()
        .any(|event| event["state"] == "connected"
            && event["videoWidth"] == 1280
            && event["videoHeight"] == 720
            && event["videoCodec"] == "vp8"
            && event["authorized"] == false));
    assert!(result
        .events
        .iter()
        .all(|event| event["authorized"] == false));
}

#[test]
fn video_callback_cancellation_drops_remaining_frames_in_packet() {
    let result = execute_video(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            video_login(&mut stream, &mut cipher, VIDEO_ADDITIONS, 640, 360);
            send_encrypted(
                &mut stream,
                &mut cipher,
                &video_message(
                    vec![
                        EncodedVideoFrame {
                            data: vec![1],
                            key: true,
                            ..Default::default()
                        },
                        EncodedVideoFrame {
                            data: vec![2],
                            ..Default::default()
                        },
                    ],
                    0,
                ),
            );
        },
        true,
    );
    assert_eq!(result.frames.len(), 1);
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "CANCELLED"));
}

#[test]
fn approved_video_rejects_permission_grant() {
    let result = execute_video(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            video_login(&mut stream, &mut cipher, VIDEO_ADDITIONS, 640, 360);
            send_permission(&mut stream, &mut cipher, true);
        },
        false,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "UNEXPECTED_PERMISSION"));
    assert!(result.frames.is_empty());
}

#[test]
fn approved_video_keeps_large_control_packets_below_64k() {
    let result = execute_video(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            video_login(&mut stream, &mut cipher, VIDEO_ADDITIONS, 640, 360);
            let mut control = Message::new();
            control.set_test_delay(Default::default());
            let mut plain = control.write_to_bytes().unwrap();
            // Unknown length-delimited protobuf field 1000 with 70,000 bytes.
            plain.extend_from_slice(&[0xc2, 0x3e, 0xf0, 0xa2, 0x04]);
            plain.extend_from_slice(&vec![0u8; 70_000]);
            let encrypted = cipher.enc(&plain);
            let header = (((encrypted.len() as u32) << 2) | 3).to_le_bytes();
            stream.write_all(&header).unwrap();
            stream.write_all(&encrypted).unwrap();
        },
        false,
    );
    assert!(result
        .events
        .iter()
        .any(|event| event["code"] == "INVALID_FRAME"));
    assert!(result.frames.is_empty());
}

#[test]
fn video_mode_rejects_invalid_capability_and_frames() {
    for (additions, width, height, frame, expected) in [
        (
            r#"{"ord_secure_host":1,"media":false,"input_scope":"none"}"#,
            640,
            360,
            None,
            "UNSUPPORTED_PEER",
        ),
        (VIDEO_ADDITIONS, 1281, 720, None, "UNSUPPORTED_PEER"),
        (
            r#"{"ord_secure_host":1,"ord_demo":1,"media":true,"video_codec":"vp8","input_scope":"none"}"#,
            640,
            360,
            None,
            "UNSUPPORTED_PEER",
        ),
        (
            VIDEO_ADDITIONS,
            640,
            360,
            Some({
                let mut message = Message::new();
                message.set_video_frame(VideoFrame {
                    union: Some(message::video_frame::Union::Vp9s(EncodedVideoFrames::new())),
                    ..Default::default()
                });
                message
            }),
            "INVALID_VIDEO",
        ),
        (
            VIDEO_ADDITIONS,
            640,
            360,
            Some(video_message(Vec::new(), 0)),
            "INVALID_VIDEO",
        ),
        (
            VIDEO_ADDITIONS,
            640,
            360,
            Some(video_message(
                vec![EncodedVideoFrame {
                    data: vec![1],
                    key: false,
                    ..Default::default()
                }],
                0,
            )),
            "INVALID_VIDEO",
        ),
        (
            VIDEO_ADDITIONS,
            640,
            360,
            Some(video_message(
                vec![EncodedVideoFrame {
                    data: vec![1],
                    key: true,
                    ..Default::default()
                }],
                1,
            )),
            "INVALID_VIDEO",
        ),
        (
            VIDEO_ADDITIONS,
            640,
            360,
            Some(video_message(
                vec![EncodedVideoFrame {
                    data: vec![1; 2 * 1024 * 1024 + 1],
                    key: true,
                    ..Default::default()
                }],
                0,
            )),
            "INVALID_VIDEO",
        ),
    ] {
        let result = execute_video(
            move |mut stream, key| {
                let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
                video_login(&mut stream, &mut cipher, additions, width, height);
                if let Some(frame) = frame {
                    let encrypted = cipher.enc(&frame.write_to_bytes().unwrap());
                    let header = (((encrypted.len() as u32) << 2) | 3).to_le_bytes();
                    stream.write_all(&header).unwrap();
                    stream.write_all(&encrypted).unwrap();
                } else {
                    assert!(receive_encrypted(&mut stream, &mut cipher).has_misc());
                }
            },
            false,
        );
        assert!(
            result.events.iter().any(|event| event["code"] == expected),
            "{expected}: {:?}",
            result.events
        );
        assert!(result.frames.is_empty());
    }
}

struct DemoCollector {
    events: Vec<Value>,
    task: *mut remote_controller_core::session::ControllerSession,
    sends: Vec<i32>,
}

unsafe extern "C" fn collect_demo(event: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut DemoCollector);
    let event: Value = serde_json::from_str(CStr::from_ptr(event).to_str().unwrap()).unwrap();
    if event["state"] == "connected" {
        context
            .sends
            .push(controller_session_send_pointer(context.task, 10, 20));
    }
    if event["state"] == "permissions_changed" {
        context.sends.push(controller_session_send_text(
            context.task,
            CString::new("hello").unwrap().as_ptr(),
        ));
        if event["authorized"] == true {
            context
                .sends
                .push(controller_session_send_pointer(context.task, 10, 20));
        }
    }
    context.events.push(event);
}

#[test]
fn persistent_connection_gates_input_and_reports_status() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut cipher = negotiated(&mut stream, &private, "123456789", 1);
        demo_login(
            &mut stream,
            &mut cipher,
            r#"{"ord_demo":1,"input_scope":"demo_window","width":800,"height":450}"#,
        );
        send_permission(&mut stream, &mut cipher, false);
        send_permission(&mut stream, &mut cipher, true);
        let first = receive_encrypted(&mut stream, &mut cipher);
        let second = receive_encrypted(&mut stream, &mut cipher);
        assert!(
            matches!(first.union, Some(message::message::Union::KeyEvent(k)) if k.seq() == "hello")
        );
        assert!(
            matches!(second.union, Some(message::message::Union::MouseEvent(m)) if m.x == 10 && m.y == 20 && m.mask == 0)
        );
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text: r#"{"ord_demo_status":1,"x":10,"y":20,"textLength":5}"#.into(),
            ..Default::default()
        });
        let mut status = Message::new();
        status.set_misc(misc);
        send_encrypted(&mut stream, &mut cipher, &status);
        send_permission(&mut stream, &mut cipher, false);
        let mut close = Message::new();
        let mut misc = Misc::new();
        misc.set_close_reason("done".into());
        close.set_misc(misc);
        send_encrypted(&mut stream, &mut cipher, &close);
    });
    let task = controller_connection_create(
        trusted_request(&endpoint, &public, "123456789").as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut context = DemoCollector {
        events: Vec::new(),
        task,
        sends: Vec::new(),
    };
    controller_session_run(
        task,
        Some(collect_demo),
        &mut context as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    assert_eq!(context.sends, [2, 2, 0, 0, 2]);
    assert!(context
        .events
        .iter()
        .any(|e| e["state"] == "demo_status" && e["x"] == 10 && e["textLength"] == 5));
    assert!(context.events.iter().any(|e| e["state"] == "connected"
        && e["authenticated"] == true
        && e["authorized"] == false));
    assert!(context
        .events
        .last()
        .is_some_and(|e| e["authenticated"] == false && e["authorized"] == false));
}

struct BusyPeerCollector {
    task: *mut remote_controller_core::session::ControllerSession,
    statuses: usize,
    enqueue_result: Option<i32>,
}

unsafe extern "C" fn collect_busy_peer(event: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut BusyPeerCollector);
    let event: Value = serde_json::from_str(CStr::from_ptr(event).to_str().unwrap()).unwrap();
    if event["state"] == "permissions_changed" && event["authorized"] == true {
        context.enqueue_result = Some(controller_session_send_pointer(context.task, 31, 41));
    }
    if event["state"] == "demo_status" {
        context.statuses += 1;
        if context.statuses == 5 {
            controller_session_cancel(context.task);
        }
    }
}

#[test]
fn continuous_inbound_status_does_not_starve_authorized_input() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut cipher = negotiated(&mut stream, &private, "123456789", 1);
        demo_login(
            &mut stream,
            &mut cipher,
            r#"{"ord_demo":1,"input_scope":"demo_window","width":800,"height":450}"#,
        );
        send_permission(&mut stream, &mut cipher, true);
        for _ in 0..5 {
            let mut misc = Misc::new();
            misc.set_chat_message(ChatMessage {
                text: r#"{"ord_demo_status":1,"x":0,"y":0,"textLength":0}"#.into(),
                ..Default::default()
            });
            let mut status = Message::new();
            status.set_misc(misc);
            send_encrypted(&mut stream, &mut cipher, &status);
        }
        let input = receive_encrypted(&mut stream, &mut cipher);
        assert!(
            matches!(input.union, Some(message::message::Union::MouseEvent(m))
            if m.x == 31 && m.y == 41 && m.mask == 0)
        );
    });
    let task = controller_connection_create(
        trusted_request(&endpoint, &public, "123456789").as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut context = BusyPeerCollector {
        task,
        statuses: 0,
        enqueue_result: None,
    };
    controller_session_run(
        task,
        Some(collect_busy_peer),
        &mut context as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    assert_eq!(context.enqueue_result, Some(0));
    assert_eq!(context.statuses, 5);
}

struct CancelCollector {
    events: Vec<Value>,
    task: *mut remote_controller_core::session::ControllerSession,
    after_cancel: Option<i32>,
}

unsafe extern "C" fn cancel_on_permission(event: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut CancelCollector);
    let event: Value = serde_json::from_str(CStr::from_ptr(event).to_str().unwrap()).unwrap();
    if event["state"] == "permissions_changed" && event["authorized"] == true {
        controller_session_cancel(context.task);
        context.after_cancel = Some(controller_session_send_pointer(context.task, 1, 2));
    }
    context.events.push(event);
}

#[test]
fn cancelling_connected_demo_immediately_disables_input() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, private) = sign::gen_keypair();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut cipher = negotiated(&mut stream, &private, "123456789", 1);
        demo_login(
            &mut stream,
            &mut cipher,
            r#"{"ord_demo":1,"input_scope":"demo_window","width":800,"height":450}"#,
        );
        send_permission(&mut stream, &mut cipher, true);
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
    });
    let task = controller_connection_create(
        trusted_request(&endpoint, &public, "123456789").as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut context = CancelCollector {
        events: Vec::new(),
        task,
        after_cancel: None,
    };
    controller_session_run(
        task,
        Some(cancel_on_permission),
        &mut context as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    assert_eq!(context.after_cancel, Some(1));
    assert!(context
        .events
        .last()
        .is_some_and(|e| e["code"] == "CANCELLED"
            && e["authenticated"] == false
            && e["authorized"] == false));
}

#[test]
fn rejects_invalid_arguments_before_connecting() {
    let password = CString::new("secret").unwrap();
    let invalid = CString::new(r#"{"endpoint":"example.org:21118","peerId":"123456789","peerPublicKey":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}"#).unwrap();
    assert!(controller_session_create(invalid.as_ptr(), password.as_ptr(), 1000).is_null());
    assert!(controller_session_create(ptr::null(), password.as_ptr(), 1000).is_null());
    assert!(
        controller_session_create(request("127.0.0.1:1").as_ptr(), password.as_ptr(), 99).is_null()
    );
}

#[test]
fn plaintext_first_frame_sends_no_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        stream.write_all(&[4, 0]).unwrap();
        let mut byte = [0u8; 1];
        assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
    });
    let password = CString::new("secret").unwrap();
    let task = controller_session_create(request(&endpoint).as_ptr(), password.as_ptr(), 1000);
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
    controller_session_cancel(task);
    controller_session_destroy(task);
    server.join().unwrap();
    assert!(events.iter().any(|e| e["state"] == "failed"));
    assert!(events
        .iter()
        .all(|e| e["authenticated"] == false && e["authorized"] == false));
}

#[test]
fn signed_v1_login_confirms_authentication_then_closes_without_operations() {
    let events = execute(|mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".to_owned(),
            challenge: "nonce".to_owned(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        let login = receive_encrypted(&mut stream, &mut cipher);
        let Some(message::message::Union::LoginRequest(login)) = login.union else {
            panic!("expected login")
        };
        let expected = Sha256::digest(
            Sha256::digest(b"correct horsesalt")
                .iter()
                .copied()
                .chain(b"nonce".iter().copied())
                .collect::<Vec<_>>(),
        );
        assert_eq!(&login.password, expected.as_slice());
        assert_eq!(login.username, "123456789");
        assert_eq!(login.my_id, "123456789");
        let option = login.option.unwrap();
        assert_eq!(
            option.disable_keyboard.enum_value().unwrap(),
            option_message::BoolOption::Yes
        );
        assert_eq!(
            option.disable_audio.enum_value().unwrap(),
            option_message::BoolOption::Yes
        );
        assert_eq!(
            option.disable_clipboard.enum_value().unwrap(),
            option_message::BoolOption::Yes
        );
        assert_eq!(
            option.disable_camera.enum_value().unwrap(),
            option_message::BoolOption::Yes
        );
        assert_eq!(
            option.enable_file_transfer.enum_value().unwrap(),
            option_message::BoolOption::No
        );
        let mut result = Message::new();
        result.set_login_response(LoginResponse {
            union: Some(login_response::Union::PeerInfo(PeerInfo::new())),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &result);
        let close = receive_encrypted(&mut stream, &mut cipher);
        assert!(
            matches!(close.union, Some(message::message::Union::Misc(misc)) if matches!(misc.union, Some(message::misc::Union::CloseReason(_))))
        );
        let mut byte = [0u8; 1];
        assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
    });
    assert!(events
        .iter()
        .any(|e| e["state"] == "authentication_confirmed"
            && e["verified"] == true
            && e["authenticated"] == true));
    assert!(events.iter().all(|e| e["authorized"] == false));
}

#[test]
fn wrong_identity_and_legacy_version_send_no_public_key() {
    for (id, version) in [("987654321", 1), ("123456789", 0)] {
        let events = execute(move |mut stream, key| {
            let (ephemeral, _) = box_::gen_keypair();
            send_message(&mut stream, &signed_identity(id, &key, &ephemeral, version));
            let mut byte = [0u8; 1];
            assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
        });
        assert!(events.iter().any(|e| e["state"] == "failed"));
        assert!(events.iter().all(|e| e["authenticated"] == false));
    }
}

#[test]
fn wrong_signing_key_sends_no_public_key() {
    let events = execute(|mut stream, _trusted_key| {
        let (_, wrong_key) = sign::gen_keypair();
        let (ephemeral, _) = box_::gen_keypair();
        send_message(
            &mut stream,
            &signed_identity("123456789", &wrong_key, &ephemeral, 1),
        );
        let mut byte = [0u8; 1];
        assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
    });
    assert!(events.iter().any(|e| e["code"] == "IDENTITY_INVALID"));
    assert!(events
        .iter()
        .all(|e| e["verified"] == false && e["authenticated"] == false));
}

#[test]
fn rejected_password_and_corrupt_ciphertext_never_authenticate() {
    let rejected = execute(|mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".to_owned(),
            challenge: "nonce".to_owned(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        assert!(matches!(
            receive_encrypted(&mut stream, &mut cipher).union,
            Some(message::message::Union::LoginRequest(_))
        ));
        let mut result = Message::new();
        result.set_login_response(LoginResponse {
            union: Some(login_response::Union::Error("Wrong Password".to_owned())),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &result);
    });
    assert!(rejected.iter().any(|e| e["code"] == "PASSWORD_REJECTED"));
    let corrupted = execute(|mut stream, key| {
        let _ = negotiated(&mut stream, &key, "123456789", 1);
        send_frame(&mut stream, &[0u8; 20]);
    });
    assert!(corrupted.iter().any(|e| e["code"] == "INVALID_CIPHERTEXT"));
    assert!(rejected
        .iter()
        .chain(corrupted.iter())
        .all(|e| e["authenticated"] == false && e["authorized"] == false));
}

#[test]
fn signed_peer_that_stalls_times_out_without_authentication() {
    let events = execute(|mut stream, key| {
        let _ = negotiated(&mut stream, &key, "123456789", 1);
        thread::sleep(Duration::from_millis(1700));
    });
    assert!(events.iter().any(|e| e["code"] == "TIMEOUT"));
    assert!(events.iter().all(|e| e["authenticated"] == false));
}

#[test]
fn second_factor_response_stays_blocked() {
    let events = execute(|mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".to_owned(),
            challenge: "nonce".to_owned(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        let _ = receive_encrypted(&mut stream, &mut cipher);
        let mut result = Message::new();
        result.set_login_response(LoginResponse {
            union: Some(login_response::Union::Error("2FA Required".to_owned())),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &result);
    });
    assert!(events
        .iter()
        .any(|e| e["state"] == "blocked" && e["code"] == "SECOND_FACTOR_REQUIRED"));
    assert!(events
        .iter()
        .all(|e| e["authenticated"] == false && e["authorized"] == false));
}

#[test]
fn cancel_after_connect_interrupts_read_without_authentication() {
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (public, _) = sign::gen_keypair();
    let (connected_tx, connected_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        connected_tx.send(()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut byte = [0u8; 1];
        assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
    });
    let password = CString::new("secret").unwrap();
    let task = controller_session_create(
        trusted_request(&endpoint, &public, "123456789").as_ptr(),
        password.as_ptr(),
        3000,
    );
    assert!(!task.is_null());
    let task_addr = task as usize;
    let worker = thread::spawn(move || {
        let mut events = Vec::<Value>::new();
        controller_session_run(
            task_addr as *mut _,
            Some(collect),
            &mut events as *mut _ as *mut c_void,
        );
        events
    });
    connected_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let cancelled_at = std::time::Instant::now();
    controller_session_cancel(task);
    let events = worker.join().unwrap();
    assert!(cancelled_at.elapsed() < Duration::from_secs(1));
    controller_session_destroy(task);
    server.join().unwrap();
    assert!(events.iter().any(|e| e["code"] == "CANCELLED"));
    assert!(events.iter().all(|e| e["authenticated"] == false));
}

#[test]
fn short_ciphertext_cannot_use_upstream_plaintext_shortcut() {
    let events = execute(|mut stream, key| {
        let _cipher = negotiated(&mut stream, &key, "123456789", 1);
        send_frame(&mut stream, &[0]);
        let mut byte = [0; 1];
        assert_eq!(stream.read(&mut byte).unwrap_or(0), 0);
    });
    assert!(events
        .iter()
        .any(|event| event["code"] == "INVALID_CIPHERTEXT"));
    assert!(events.iter().all(|event| event["authenticated"] == false));
}

#[test]
fn replayed_encrypted_challenge_is_rejected_after_login_request() {
    let events = execute(|mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".into(),
            challenge: "nonce".into(),
            ..Default::default()
        });
        let sealed = cipher.enc(&challenge.write_to_bytes().unwrap());
        send_frame(&mut stream, &sealed);
        let _ = receive_encrypted(&mut stream, &mut cipher);
        send_frame(&mut stream, &sealed);
    });
    assert!(events
        .iter()
        .any(|event| event["code"] == "INVALID_CIPHERTEXT"));
    assert!(events
        .iter()
        .all(|event| event["authenticated"] == false && event["authorized"] == false));
}

#[test]
fn empty_password_waits_for_explicit_peer_confirmation() {
    let events = execute_with_password("", |mut stream, key| {
        let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
        let mut challenge = Message::new();
        challenge.set_hash(Hash {
            salt: "salt".into(),
            challenge: "nonce".into(),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &challenge);
        let login = receive_encrypted(&mut stream, &mut cipher);
        assert!(
            matches!(login.union, Some(message::message::Union::LoginRequest(request)) if request.password.is_empty())
        );
        let mut result = Message::new();
        result.set_login_response(LoginResponse {
            union: Some(login_response::Union::PeerInfo(PeerInfo::new())),
            ..Default::default()
        });
        send_encrypted(&mut stream, &mut cipher, &result);
        let _ = receive_encrypted(&mut stream, &mut cipher);
    });
    let waiting = events
        .iter()
        .position(|event| event["state"] == "awaiting_approval")
        .unwrap();
    let confirmed = events
        .iter()
        .position(|event| event["state"] == "authentication_confirmed")
        .unwrap();
    assert!(waiting < confirmed);
    assert!(events[..confirmed]
        .iter()
        .all(|event| event["authenticated"] == false));
    assert!(events.iter().all(|event| event["authorized"] == false));
}
