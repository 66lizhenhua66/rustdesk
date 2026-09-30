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
    controller_session_cancel, controller_session_create, controller_session_destroy,
    controller_session_run,
};
use remote_controller_core::{
    protos::{
        message::{
            self, login_response, option_message, Hash, LoginResponse, Message, PeerInfo, SignedId,
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
