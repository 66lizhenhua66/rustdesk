use std::{
    ffi::{c_char, c_void, CStr, CString},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use remote_controller_core::session::{
    controller_connection_create, controller_session_cancel, controller_session_destroy,
    controller_session_run, controller_session_send_pointer, controller_session_send_text,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sodiumoxide::crypto::sign;
use windows_demo_host::{DemoServer, HostHandle, HostState};

unsafe extern "C" fn callback(value: *const c_char, user: *mut c_void) {
    if let Ok(text) = CStr::from_ptr(value).to_str() {
        if let Ok(event) = serde_json::from_str(text) {
            (*(user as *const Mutex<Vec<Value>>))
                .lock()
                .unwrap()
                .push(event);
        }
    }
}

fn until<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(value) = f() {
            return value;
        }
        assert!(Instant::now() < deadline, "state was not reached");
        thread::sleep(Duration::from_millis(20));
    }
}

fn request(server: &DemoServer) -> CString {
    let profile: Value = serde_json::from_str(server.public_profile_json()).unwrap();
    CString::new(
        json!({
            "endpoint":server.local_addr().to_string(),
            "peerId":profile["peerId"],
            "peerPublicKey":profile["peerPublicKey"],
            "peerFingerprint":profile["peerFingerprint"],
            "minimumKxVersion":1,
        })
        .to_string(),
    )
    .unwrap()
}

fn pending(handle: &HostHandle) -> u64 {
    until(|| {
        let snapshot = handle.snapshot();
        (snapshot.state == HostState::Pending)
            .then_some(snapshot.session_id)
            .flatten()
    })
}

struct Client {
    task: *mut remote_controller_core::session::ControllerSession,
    events: *mut Mutex<Vec<Value>>,
    runner: Option<thread::JoinHandle<()>>,
}

impl Client {
    fn start(request: CString) -> Self {
        let events = Box::into_raw(Box::new(Mutex::new(Vec::<Value>::new())));
        let task = controller_connection_create(request.as_ptr(), c"".as_ptr(), 10_000);
        assert!(!task.is_null());
        let task_value = task as usize;
        let events_value = events as usize;
        let runner = thread::spawn(move || {
            controller_session_run(
                task_value as *mut _,
                Some(callback),
                events_value as *mut c_void,
            )
        });
        Self {
            task,
            events,
            runner: Some(runner),
        }
    }
    fn events(&self) -> Vec<Value> {
        unsafe { &*self.events }.lock().unwrap().clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        controller_session_cancel(self.task);
        if let Some(runner) = self.runner.take() {
            runner.join().unwrap();
        }
        controller_session_destroy(self.task);
        unsafe {
            drop(Box::from_raw(self.events));
        }
    }
}

#[test]
fn approved_session_updates_only_demo_state_and_revoke_blocks_input() {
    let mut server = DemoServer::start("127.0.0.1:0".parse().unwrap()).unwrap();
    let handle = server.handle();
    let events = Box::new(Mutex::new(Vec::<Value>::new()));
    let events_ptr = Box::into_raw(events);
    let task = controller_connection_create(request(&server).as_ptr(), c"".as_ptr(), 10_000);
    assert!(!task.is_null());
    let task_value = task as usize;
    let events_value = events_ptr as usize;
    let runner = thread::spawn(move || {
        controller_session_run(
            task_value as *mut _,
            Some(callback),
            events_value as *mut c_void,
        )
    });
    let id = until(|| {
        let snapshot = handle.snapshot();
        if snapshot.state == HostState::Pending {
            return snapshot.session_id;
        }
        let current = unsafe { &*events_ptr }.lock().unwrap();
        if current.iter().any(|v| v["state"] == "failed") {
            panic!("controller failed: {current:?}");
        }
        None
    });
    let code = handle.snapshot().pair_code;
    until(|| {
        let events = unsafe { &*events_ptr }.lock().unwrap();
        events
            .iter()
            .find(|v| v["state"] == "awaiting_approval")
            .cloned()
    });
    assert!(unsafe { &*events_ptr }
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["confirmationCode"] == code));
    assert!(handle.approve(id));
    until(|| (handle.snapshot().state == HostState::Active).then_some(()));
    assert_eq!(controller_session_send_pointer(task, 10, 20), 2);
    assert!(handle.allow_input(id));
    until(|| {
        let events = unsafe { &*events_ptr }.lock().unwrap();
        events
            .iter()
            .any(|v| v["state"] == "permissions_changed" && v["authorized"] == true)
            .then_some(())
    });
    assert_eq!(controller_session_send_pointer(task, 12, 34), 0);
    assert_eq!(controller_session_send_text(task, c"demo".as_ptr()), 0);
    until(|| {
        let snapshot = handle.snapshot();
        (snapshot.x == 12 && snapshot.y == 34 && snapshot.text == "demo").then_some(())
    });
    let prior_events = unsafe { &*events_ptr }.lock().unwrap().len();
    assert!(handle.revoke_input(id));
    until(|| {
        let events = unsafe { &*events_ptr }.lock().unwrap();
        events[prior_events..]
            .iter()
            .any(|v| {
                v["state"] == "permissions_changed"
                    && v["authorized"] == false
                    && v["authenticated"] == true
            })
            .then_some(())
    });
    assert_eq!(controller_session_send_pointer(task, 40, 50), 2);
    assert_eq!(handle.snapshot().x, 12);
    assert!(!handle.approve(id));
    controller_session_cancel(task);
    runner.join().unwrap();
    controller_session_destroy(task);
    unsafe {
        drop(Box::from_raw(events_ptr));
    }
    server.stop();
}

#[test]
fn wildcard_listen_is_rejected() {
    assert!(DemoServer::start("0.0.0.0:0".parse().unwrap()).is_err());
}

#[test]
fn rejection_and_timeout_never_authorize_old_session() {
    let mut server = DemoServer::start_with_approval_timeout(
        "127.0.0.1:0".parse().unwrap(),
        Duration::from_millis(200),
    )
    .unwrap();
    let handle = server.handle();
    let first = Client::start(request(&server));
    let first_id = pending(&handle);
    assert!(handle.reject(first_id));
    until(|| (handle.snapshot().state == HostState::Listening).then_some(()));
    assert!(!handle.approve(first_id));
    drop(first);
    let second = Client::start(request(&server));
    let second_id = pending(&handle);
    assert_ne!(first_id, second_id);
    assert!(!handle.approve(first_id));
    until(|| (handle.snapshot().state == HostState::Listening).then_some(()));
    assert!(!handle.approve(second_id));
    assert!(handle.poll_event().is_some());
    drop(second);
    server.stop();
}

#[test]
fn new_connection_starts_read_only_after_prior_grant_and_disconnect() {
    let mut server = DemoServer::start("127.0.0.1:0".parse().unwrap()).unwrap();
    let handle = server.handle();
    let first = Client::start(request(&server));
    let first_id = pending(&handle);
    assert!(handle.approve(first_id));
    until(|| (handle.snapshot().state == HostState::Active).then_some(()));
    assert!(handle.allow_input(first_id));
    until(|| handle.snapshot().input_allowed.then_some(()));
    assert!(handle.disconnect(first_id));
    until(|| (handle.snapshot().state == HostState::Listening).then_some(()));
    assert_eq!(controller_session_send_pointer(first.task, 3, 4), 1);
    drop(first);
    let second = Client::start(request(&server));
    let second_id = pending(&handle);
    assert_ne!(first_id, second_id);
    assert!(!handle.approve(first_id));
    assert!(handle.approve(second_id));
    until(|| (handle.snapshot().state == HostState::Active).then_some(()));
    assert!(!handle.snapshot().input_allowed);
    assert_eq!(controller_session_send_pointer(second.task, 3, 4), 2);
    drop(second);
    server.stop();
}

#[test]
fn wrong_trusted_key_fails_before_local_approval() {
    let mut server = DemoServer::start("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut value: Value = serde_json::from_str(request(&server).to_str().unwrap()).unwrap();
    sodiumoxide::init().unwrap();
    let (wrong, _) = sign::gen_keypair();
    value["peerPublicKey"] = STANDARD.encode(wrong.0).into();
    value["peerFingerprint"] = format!("{:x}", Sha256::digest(wrong.0)).into();
    let client = Client::start(CString::new(value.to_string()).unwrap());
    until(|| {
        client
            .events()
            .iter()
            .find(|event| event["code"] == "IDENTITY_INVALID")
            .cloned()
    });
    assert_ne!(server.handle().snapshot().state, HostState::Pending);
    drop(client);
    server.stop();
}
