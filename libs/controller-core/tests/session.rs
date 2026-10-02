use std::{
    ffi::{c_char, c_void, CStr, CString},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    ptr,
    sync::mpsc,
    thread,
    time::Duration,
};

use bytes::BytesMut;
use protobuf::Message as _;
use remote_controller_core::session::{
    approval_code, controller_connection_create, controller_session_cancel,
    controller_session_create, controller_session_destroy, controller_session_run,
    controller_session_send_pointer, controller_session_send_text,
};
use remote_controller_core::{
    protos::{
        message::{
            self, login_response, option_message, permission_info, ChatMessage, Hash,
            LoginResponse, Message, Misc, PeerInfo, PermissionInfo, SignedId,
        },
        rendezvous::IdPk,
    },
    upstream_crypto::{Encrypt, KxTranscript},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sodiumoxide::crypto::{box_, sign};

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
    let task = controller_connection_create(
        trusted_request(&endpoint, &public, "123456789").as_ptr(),
        CString::new("").unwrap().as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    let mut events = Vec::<Value>::new();
    controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
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
    let events = execute_demo(|mut stream, key| {
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
