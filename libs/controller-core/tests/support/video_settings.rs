use super::*;
use remote_controller_core::session::controller_session_set_video_settings_v1;

const SETTINGS_ADDITIONS: &str = r#"{"ord_secure_host":1,"media":true,"video_codec":"vp8","input_scope":"windows_primary","input_version":2,"video_settings_version":1}"#;

struct Collector {
    video: VideoCollector,
    codes: Vec<i32>,
    runtime: bool,
}

unsafe extern "C" fn event(raw: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut Collector);
    let value: Value = serde_json::from_str(CStr::from_ptr(raw).to_str().unwrap()).unwrap();
    if value["state"] == "connected" && context.runtime {
        context.codes.push(controller_session_set_video_settings_v1(
            context.video.task,
            c"{\"quality\":\"high\",\"fps\":30}".as_ptr(),
        ));
        context.codes.push(controller_session_set_video_settings_v1(
            context.video.task,
            c"{\"quality\":\"high\",\"fps\":30,\"extra\":0}".as_ptr(),
        ));
    }
    context.video.events.push(value);
}

unsafe extern "C" fn frame(
    data: *const u8,
    length: u32,
    width: u32,
    height: u32,
    pts: i64,
    key: u8,
    user: *mut c_void,
) {
    let context = &mut *(user as *mut Collector);
    collect_video(
        data,
        length,
        width,
        height,
        pts,
        key,
        &mut context.video as *mut _ as *mut c_void,
    );
    if context.runtime && context.video.frames.len() == 1 {
        for _ in 0..2 {
            context.codes.push(controller_session_set_video_settings_v1(
                context.video.task,
                c"{\"quality\":\"high\",\"fps\":30}".as_ptr(),
            ));
        }
    } else if context.video.frames.len() == 2 {
        controller_session_cancel(context.video.task);
    }
}

fn execute<F>(server_fn: F, runtime: bool) -> Collector
where
    F: FnOnce(TcpStream, sign::SecretKey) + Send + 'static,
{
    sodiumoxide::init().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let (public, private) = sign::gen_keypair();
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
    request["expectedPeer"] = json!("secure_control");
    request["videoQuality"] = json!("balanced");
    request["videoFps"] = json!(15);
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        c"".as_ptr(),
        1500,
    );
    assert!(!task.is_null());
    assert_eq!(
        controller_session_set_video_settings_v1(
            task,
            c"{\"quality\":\"low\",\"fps\":10}".as_ptr()
        ),
        1
    );
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        server_fn(stream, private);
    });
    let mut context = Collector {
        video: VideoCollector {
            events: Vec::new(),
            frames: Vec::new(),
            task,
            cancel_after_first: false,
        },
        codes: Vec::new(),
        runtime,
    };
    controller_session_run_video(
        task,
        Some(event),
        Some(frame),
        &mut context as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    server.join().unwrap();
    context
}

fn login(stream: &mut TcpStream, cipher: &mut Encrypt, additions: &str) {
    let mut challenge = Message::new();
    challenge.set_hash(Hash {
        salt: "salt".into(),
        challenge: "nonce".into(),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &challenge);
    let request = receive_encrypted(stream, cipher);
    let settings = request
        .login_request()
        .ord_video_settings
        .as_ref()
        .expect("initial settings");
    assert_eq!(
        (
            settings.version,
            settings.request_id,
            settings.quality.as_str(),
            settings.fps
        ),
        (1, 1, "balanced", 15)
    );
    let mut response = Message::new();
    response.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo {
            platform_additions: additions.into(),
            displays: vec![DisplayInfo {
                width: 1920,
                height: 1080,
                online: true,
                ..Default::default()
            }],
            ..Default::default()
        })),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &response);
}

fn state(id: u64, quality: &str, fps: u32, width: u32, height: u32) -> message::OrdVideoState {
    message::OrdVideoState {
        version: 1,
        request_id: id,
        quality: quality.into(),
        fps,
        width,
        height,
        ..Default::default()
    }
}

fn send_state(stream: &mut TcpStream, cipher: &mut Encrypt, state: message::OrdVideoState) {
    let mut message = Message::new();
    message.set_ord_video_state(state);
    send_encrypted(stream, cipher, &message);
}

fn send_frame(stream: &mut TcpStream, cipher: &mut Encrypt, key: bool) {
    send_encrypted(
        stream,
        cipher,
        &video_message(
            vec![EncodedVideoFrame {
                data: vec![1, 2, 3],
                key,
                ..Default::default()
            }],
            0,
        ),
    );
}

#[test]
fn initial_ack_and_runtime_settings_update_dimensions_without_input_permission() {
    let result = execute(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            login(&mut stream, &mut cipher, SETTINGS_ADDITIONS);
            send_initial_control_state(&mut stream, &mut cipher);
            send_state(
                &mut stream,
                &mut cipher,
                state(1, "balanced", 15, 1920, 1080),
            );
            send_frame(&mut stream, &mut cipher, true);
            let request = receive_encrypted(&mut stream, &mut cipher);
            let settings = request.ord_video_settings();
            assert_eq!(
                (
                    settings.version,
                    settings.request_id,
                    settings.quality.as_str(),
                    settings.fps
                ),
                (1, 2, "high", 30)
            );
            send_state(&mut stream, &mut cipher, state(2, "high", 30, 2560, 1440));
            send_frame(&mut stream, &mut cipher, true);
            let _ = stream.read(&mut [0]);
        },
        true,
    );
    assert_eq!(result.codes, [4, 3, 0, 4]);
    assert_eq!(
        result
            .video
            .frames
            .iter()
            .map(|frame| (frame.1, frame.2, frame.4))
            .collect::<Vec<_>>(),
        [(1920, 1080, 1), (2560, 1440, 1)]
    );
    let states: Vec<_> = result
        .video
        .events
        .iter()
        .filter(|event| event["state"] == "video_settings")
        .collect();
    assert_eq!(states.len(), 2);
    assert_eq!(states[1]["videoRequestId"], 2);
    assert_eq!(states[1]["videoQuality"], "high");
    assert_eq!(states[1]["videoFps"], 30);
    assert!(states
        .iter()
        .all(|event| event["videoSettingsSupported"] == true
            && event["authenticated"] == true
            && event["verified"] == true
            && event["authorized"] == false));
}

#[test]
fn invalid_or_missing_settings_ack_and_nonkeyframe_fail_closed() {
    for (case, code) in [
        ("unsupported", "UNSUPPORTED_VIDEO_SETTINGS"),
        ("before_ack", "UNEXPECTED_VIDEO"),
        ("wrong_id", "INVALID_VIDEO_SETTINGS"),
        ("wrong_quality", "INVALID_VIDEO_SETTINGS"),
        ("wrong_fps", "INVALID_VIDEO_SETTINGS"),
        ("oversized", "INVALID_VIDEO_SETTINGS"),
        ("odd", "INVALID_VIDEO_SETTINGS"),
        ("unknown", "INVALID_VIDEO_SETTINGS"),
        ("unsolicited", "INVALID_VIDEO_SETTINGS"),
        ("nonkeyframe", "INVALID_VIDEO"),
        ("runtime_nonkeyframe", "INVALID_VIDEO"),
        ("timeout", "VIDEO_SETTINGS_TIMEOUT"),
    ] {
        let result = execute(
            move |mut stream, key| {
                let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
                login(
                    &mut stream,
                    &mut cipher,
                    if case == "unsupported" {
                        CONTROL_ADDITIONS
                    } else {
                        SETTINGS_ADDITIONS
                    },
                );
                if case == "before_ack" {
                    send_frame(&mut stream, &mut cipher, true);
                } else if !matches!(case, "unsupported" | "timeout") {
                    let mut ack = state(1, "balanced", 15, 1920, 1080);
                    match case {
                        "wrong_id" => ack.request_id = 2,
                        "wrong_quality" => ack.quality = "low".into(),
                        "wrong_fps" => ack.fps = 30,
                        "oversized" => ack.width = 1922,
                        "odd" => ack.height = 1079,
                        "unknown" => ack.special_fields.mut_unknown_fields().add_varint(99, 1),
                        _ => {}
                    }
                    send_state(&mut stream, &mut cipher, ack);
                    if case == "unsolicited" {
                        send_state(
                            &mut stream,
                            &mut cipher,
                            state(1, "balanced", 15, 1920, 1080),
                        );
                    }
                    if case == "nonkeyframe" {
                        send_frame(&mut stream, &mut cipher, false);
                    }
                    if case == "runtime_nonkeyframe" {
                        send_frame(&mut stream, &mut cipher, true);
                        assert!(
                            receive_encrypted(&mut stream, &mut cipher).has_ord_video_settings()
                        );
                        send_state(&mut stream, &mut cipher, state(2, "high", 30, 2560, 1440));
                        send_frame(&mut stream, &mut cipher, false);
                    }
                }
                let _ = stream.read(&mut [0]);
            },
            case == "runtime_nonkeyframe",
        );
        assert_eq!(
            result.video.frames.len(),
            usize::from(case == "runtime_nonkeyframe"),
            "{case}"
        );
        assert!(
            result
                .video
                .events
                .iter()
                .any(|event| event["code"] == code),
            "{case}: {:?}",
            result.video.events
        );
    }
}
