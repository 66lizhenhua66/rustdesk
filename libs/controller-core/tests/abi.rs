use std::{
    ffi::{CStr, CString},
    io::{Read, Write},
    net::TcpListener,
    sync::Mutex,
    thread,
    time::Duration,
};

use remote_controller_core::{
    controller_free_string, controller_probe_cancel, controller_probe_create,
    controller_probe_destroy, controller_probe_run, controller_validate_profile,
};

fn validate(input: &str) -> serde_json::Value {
    let input = CString::new(input).unwrap();
    let result = controller_validate_profile(input.as_ptr());
    assert!(!result.is_null());
    let value = serde_json::from_str(unsafe { CStr::from_ptr(result) }.to_str().unwrap()).unwrap();
    controller_free_string(result);
    value
}

#[test]
fn direct_profile_normalizes_literal_address_and_rejects_secrets() {
    let valid = validate(
        r#"{"id":"","name":"Office","mode":"direct","target":"[::1]","server":"","serverKey":"","peerFingerprint":""}"#,
    );
    assert_eq!(valid["profile"]["target"], "[::1]:21118");
    assert_eq!(valid["readyForSession"], false);
    assert_eq!(valid["reason"], "REMOTE_SESSION_NOT_IMPLEMENTED");
    let invalid = validate(
        r#"{"id":"","name":"Office","mode":"direct","target":"127.0.0.1","server":"","serverKey":"","peerFingerprint":"","password":"secret"}"#,
    );
    assert_eq!(invalid["ok"], false);
}

#[test]
fn server_key_and_fingerprint_require_valid_public_material() {
    let bad_key = validate(
        r#"{"id":"a","name":"Office","mode":"relay","target":"123456","server":"example.com","serverKey":"bad","peerFingerprint":""}"#,
    );
    assert_eq!(bad_key["ok"], false);
    let bad_fingerprint = validate(
        r#"{"id":"a","name":"Office","mode":"direct","target":"127.0.0.1","server":"","serverKey":"","peerFingerprint":"abcd"}"#,
    );
    assert_eq!(bad_fingerprint["ok"], false);
}

#[test]
fn direct_profile_preserves_explicit_peer_identity_and_rejects_bad_keys() {
    let public_key = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [7u8; 32]);
    let profile = serde_json::json!({
        "id":"local", "name":"Pinned", "mode":"direct", "target":"127.0.0.1",
        "server":"", "serverKey":"", "peerFingerprint":"",
        "peerId":"123456789", "peerPublicKey":public_key
    });
    let valid = validate(&profile.to_string());
    assert_eq!(valid["ok"], true);
    assert_eq!(valid["profile"]["peerId"], "123456789");
    assert_eq!(valid["profile"]["peerPublicKey"], profile["peerPublicKey"]);
    let mut bad = profile;
    bad["peerPublicKey"] = "not-a-key".into();
    assert_eq!(validate(&bad.to_string())["code"], "INVALID_PEER_KEY");
}

unsafe extern "C" fn collect(json: *const std::ffi::c_char, user: *mut std::ffi::c_void) {
    let events = &*(user as *const Mutex<Vec<serde_json::Value>>);
    let text = CStr::from_ptr(json).to_str().unwrap();
    events
        .lock()
        .unwrap()
        .push(serde_json::from_str(text).unwrap());
}

#[test]
fn loopback_probe_is_read_only_and_never_authorizes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = CString::new(listener.local_addr().unwrap().to_string()).unwrap();
    let events = Mutex::new(Vec::<serde_json::Value>::new());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let mut received = [0; 1];
        assert!(matches!(stream.read(&mut received), Err(_) | Ok(0)));
        stream.write_all(&[20, 0x1a, 3, 0x0a, 1, b'a']).unwrap();
    });
    let task = controller_probe_create(endpoint.as_ptr(), 600);
    assert!(!task.is_null());
    controller_probe_run(task, Some(collect), &events as *const _ as *mut _);
    controller_probe_destroy(task);
    server.join().unwrap();
    let events = events.into_inner().unwrap();
    assert!(events.iter().any(|v| v["state"] == "transport_open"));
    assert!(events.iter().any(|v| v["state"] == "protocol_message"));
    assert!(events
        .iter()
        .all(|v| v["verified"] == false && v["authorized"] == false));
    assert_eq!(events.last().unwrap()["state"], "blocked");
}

#[test]
fn empty_and_unknown_protobuf_frames_do_not_claim_protocol_identity() {
    for frame in [vec![0], vec![12, 0xa0, 6, 1]] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = CString::new(listener.local_addr().unwrap().to_string()).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(&frame).unwrap();
        });
        let events = Mutex::new(Vec::<serde_json::Value>::new());
        let task = controller_probe_create(endpoint.as_ptr(), 300);
        controller_probe_run(task, Some(collect), &events as *const _ as *mut _);
        controller_probe_destroy(task);
        server.join().unwrap();
        let events = events.into_inner().unwrap();
        assert!(!events.iter().any(|v| v["state"] == "protocol_message"));
        assert_eq!(events.last().unwrap()["state"], "blocked");
    }
}

#[test]
fn oversized_first_frame_is_rejected_without_authorization() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = CString::new(listener.local_addr().unwrap().to_string()).unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let oversized = (65537u32 << 2) | 3;
        stream.write_all(&oversized.to_le_bytes()).unwrap();
    });
    let events = Mutex::new(Vec::<serde_json::Value>::new());
    let task = controller_probe_create(endpoint.as_ptr(), 300);
    controller_probe_run(task, Some(collect), &events as *const _ as *mut _);
    controller_probe_destroy(task);
    server.join().unwrap();
    let events = events.into_inner().unwrap();
    assert_eq!(events.last().unwrap()["code"], "FRAME_TOO_LARGE");
    assert!(events
        .iter()
        .all(|v| v["verified"] == false && v["authorized"] == false));
}

#[test]
fn cancel_before_run_reports_cancelled() {
    let endpoint = CString::new("127.0.0.1:21118").unwrap();
    let task = controller_probe_create(endpoint.as_ptr(), 1000);
    let events = Mutex::new(Vec::<serde_json::Value>::new());
    controller_probe_cancel(task);
    controller_probe_run(task, Some(collect), &events as *const _ as *mut _);
    controller_probe_destroy(task);
    assert_eq!(
        events.into_inner().unwrap().last().unwrap()["state"],
        "cancelled"
    );
}

#[test]
fn cancel_stops_a_connected_probe_waiting_for_a_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = CString::new(listener.local_addr().unwrap().to_string()).unwrap();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        accepted_tx.send(()).unwrap();
        let mut received = [0; 1];
        assert_eq!(stream.read(&mut received).unwrap(), 0);
    });
    let events = Mutex::new(Vec::<serde_json::Value>::new());
    let task = controller_probe_create(endpoint.as_ptr(), 2000);
    assert!(!task.is_null());
    let task_address = task as usize;
    let event_address = &events as *const _ as usize;
    let (finished_tx, finished_rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        controller_probe_run(
            task_address as *mut _,
            Some(collect),
            event_address as *mut _,
        );
        finished_tx.send(()).unwrap();
    });
    accepted_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    controller_probe_cancel(task);
    finished_rx
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
    worker.join().unwrap();
    controller_probe_destroy(task);
    server.join().unwrap();
    let events = events.into_inner().unwrap();
    assert_eq!(events.last().unwrap()["state"], "cancelled");
    assert!(events.iter().all(|event| event["authorized"] == false));
}
