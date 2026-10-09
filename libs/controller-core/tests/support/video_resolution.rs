use super::*;
use remote_controller_core::session::controller_session_set_video_settings_v1;

const ADDITIONS: &str = r#"{"ord_secure_host":1,"media":true,"video_codec":"vp8","input_scope":"windows_primary","input_version":2,"video_settings_version":2}"#;
const SYNC: &CStr = c"{\"quality\":\"high\",\"fps\":30,\"resolutionMode\":\"sync\",\"resolutionWidth\":1920,\"resolutionHeight\":1080}";
const ORIGINAL: &CStr = c"{\"quality\":\"low\",\"fps\":10,\"resolutionMode\":\"sync\",\"resolutionWidth\":0,\"resolutionHeight\":0}";

struct Collector {
    video: VideoCollector,
    codes: Vec<i32>,
    control: bool,
}

unsafe extern "C" fn event(raw: *const c_char, user: *mut c_void) {
    let context = &mut *(user as *mut Collector);
    let value: Value = serde_json::from_str(CStr::from_ptr(raw).to_str().unwrap()).unwrap();
    if context.control && value["state"] == "video_settings" && value["videoRequestId"] == 1 {
        context.codes.push(controller_session_set_input_enabled_v1(
            context.video.task,
            1,
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
    let task = context.video.task;
    if !context.control {
        context.codes.push(controller_session_set_video_settings_v1(
            task,
            SYNC.as_ptr(),
        ));
        controller_session_cancel(task);
    } else if context.video.frames.len() == 1 {
        context.codes.push(controller_session_send_input_v1(
            task,
            c"{\"kind\":\"move\",\"x\":1,\"y\":2}".as_ptr(),
        ));
        context.codes.push(controller_session_set_video_settings_v1(
            task,
            SYNC.as_ptr(),
        ));
        context.codes.push(controller_session_send_input_v1(
            task,
            c"{\"kind\":\"move\",\"x\":3,\"y\":4}".as_ptr(),
        ));
        context
            .codes
            .push(controller_session_set_input_enabled_v1(task, 1));
    } else if context.video.frames.len() == 2 {
        context.codes.push(controller_session_set_video_settings_v1(
            task,
            ORIGINAL.as_ptr(),
        ));
    } else {
        context.codes.push(controller_session_send_input_v1(
            task,
            c"{\"kind\":\"key\",\"code\":\"KeyA\",\"down\":false}".as_ptr(),
        ));
        controller_session_cancel(task);
    }
}

fn execute<F>(server_fn: F, control: bool) -> Collector
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
    request["videoResolutionMode"] = json!("preserve");
    request["videoResolutionWidth"] = json!(1280);
    request["videoResolutionHeight"] = json!(720);
    let task = controller_connection_create(
        CString::new(request.to_string()).unwrap().as_ptr(),
        c"".as_ptr(),
        1500,
    );
    assert!(!task.is_null());
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
        control,
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

fn login(stream: &mut TcpStream, cipher: &mut Encrypt) {
    let mut challenge = Message::new();
    challenge.set_hash(Hash {
        salt: "salt".into(),
        challenge: "nonce".into(),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &challenge);
    let request = receive_encrypted(stream, cipher);
    let settings = request.login_request().ord_video_settings.as_ref().unwrap();
    assert_eq!(
        (
            settings.version,
            settings.request_id,
            settings.resolution_mode.as_str(),
            settings.resolution_width,
            settings.resolution_height
        ),
        (2, 1, "preserve", 1280, 720)
    );
    let mut response = Message::new();
    response.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo {
            platform_additions: ADDITIONS.into(),
            displays: vec![DisplayInfo {
                width: 1280,
                height: 720,
                online: true,
                ..Default::default()
            }],
            ..Default::default()
        })),
        ..Default::default()
    });
    send_encrypted(stream, cipher, &response);
    send_initial_control_state(stream, cipher);
}

fn initial() -> message::OrdVideoState {
    message::OrdVideoState {
        version: 2,
        request_id: 1,
        quality: "balanced".into(),
        fps: 15,
        width: 1280,
        height: 720,
        resolution_mode: "preserve".into(),
        resolution_width: 1280,
        resolution_height: 720,
        desktop_width: 3840,
        desktop_height: 2160,
        original_width: 3840,
        original_height: 2160,
        supported_resolutions: [(1280, 720), (1920, 1080), (3840, 2160)]
            .into_iter()
            .map(|(width, height)| message::Resolution {
                width,
                height,
                ..Default::default()
            })
            .collect(),
        resolution_sync_supported: true,
        ..Default::default()
    }
}

fn send_state(stream: &mut TcpStream, cipher: &mut Encrypt, state: message::OrdVideoState) {
    let mut message = Message::new();
    message.set_ord_video_state(state);
    send_encrypted(stream, cipher, &message);
}

fn send_frame(stream: &mut TcpStream, cipher: &mut Encrypt) {
    send_encrypted(
        stream,
        cipher,
        &video_message(
            vec![EncodedVideoFrame {
                data: vec![1, 2, 3],
                key: true,
                ..Default::default()
            }],
            0,
        ),
    );
}

fn grant(stream: &mut TcpStream, cipher: &mut Encrypt, request_id: u64, token: u8) {
    let mut response = Message::new();
    response.set_ord_input_state(message::OrdInputState {
        version: 1,
        supported: true,
        enabled: true,
        grant_token: vec![token; 16],
        request_id,
        ..Default::default()
    });
    send_encrypted(stream, cipher, &response);
}

#[test]
fn sync_releases_pending_input_and_business_failure_keeps_actual_stream_and_grant() {
    let result = execute(
        |mut stream, key| {
            let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
            login(&mut stream, &mut cipher);
            send_state(&mut stream, &mut cipher, initial());
            let input = receive_encrypted(&mut stream, &mut cipher);
            let request_id = input.ord_input_request().request_id;
            assert!(input.ord_input_request().enabled);
            grant(&mut stream, &mut cipher, request_id, 1);
            send_frame(&mut stream, &mut cipher);
            for (id, token) in [(2, 1), (3, 2)] {
                let release = receive_encrypted(&mut stream, &mut cipher);
                assert!(release.ord_input_event().has_release_all());
                assert_eq!(release.ord_input_event().grant_token, vec![token; 16]);
                let request = receive_encrypted(&mut stream, &mut cipher);
                let settings = request.ord_video_settings();
                assert_eq!(
                    (
                        settings.version,
                        settings.request_id,
                        settings.resolution_mode.as_str()
                    ),
                    (2, id, "sync")
                );
                assert_eq!(
                    (settings.resolution_width, settings.resolution_height),
                    if id == 2 { (1920, 1080) } else { (0, 0) }
                );
                if id == 2 {
                    for _ in 0..6 {
                        assert!(receive_encrypted(&mut stream, &mut cipher)
                            .ord_input_event()
                            .has_keep_alive());
                    }
                }
                grant(&mut stream, &mut cipher, request_id, token + 1);
                let mut ack = initial();
                ack.request_id = id;
                ack.quality = "high".into();
                ack.fps = 30;
                ack.resolution_mode = "sync".into();
                ack.resolution_width = 1920;
                ack.resolution_height = 1080;
                ack.width = 1920;
                ack.height = 1080;
                ack.desktop_width = 1920;
                ack.desktop_height = 1080;
                if id == 3 {
                    ack.error_code = "DISPLAY_SWITCH_FAILED".into();
                }
                send_state(&mut stream, &mut cipher, ack);
                send_frame(&mut stream, &mut cipher);
            }
            let _ = stream.read(&mut [0]);
        },
        true,
    );
    assert_eq!(result.codes, [0, 0, 0, 4, 4, 0, 0]);
    assert_eq!(
        result
            .video
            .frames
            .iter()
            .map(|frame| (frame.1, frame.2))
            .collect::<Vec<_>>(),
        [(1280, 720), (1920, 1080), (1920, 1080)]
    );
    let states: Vec<_> = result
        .video
        .events
        .iter()
        .filter(|event| event["state"] == "video_settings")
        .collect();
    assert_eq!(states.len(), 3, "{:?}", result.video.events);
    assert_eq!(states[2]["videoSettingsError"], "DISPLAY_SWITCH_FAILED");
    assert_eq!(states[2]["videoResolutionMode"], "sync");
    assert_eq!(states[2]["videoQuality"], "high");
    assert_eq!(states[2]["authorized"], true);
    assert_eq!(states[2]["originalWidth"], 3840);
}

#[test]
fn v2_rejects_invalid_acknowledgments_and_does_not_sync_without_capability() {
    for case in [
        "wrong_id",
        "initial_error",
        "invalid_resolution",
        "invalid_mode",
        "oversized_mode",
        "unavailable",
        "unsupported_mode",
    ] {
        let result = execute(
            move |mut stream, key| {
                let mut cipher = negotiated(&mut stream, &key, "123456789", 1);
                login(&mut stream, &mut cipher);
                let mut ack = initial();
                match case {
                    "wrong_id" => ack.request_id = 2,
                    "initial_error" => ack.error_code = "DISPLAY_BUSY".into(),
                    "invalid_resolution" => ack.resolution_width = 1920,
                    "invalid_mode" => ack.resolution_mode = "sync".into(),
                    "oversized_mode" => ack.supported_resolutions[0].width = 5000,
                    "unavailable" => {
                        ack.resolution_sync_supported = false;
                        ack.supported_resolutions.clear();
                    }
                    "unsupported_mode" => {
                        ack.supported_resolutions.retain(|mode| mode.width != 1920)
                    }
                    _ => unreachable!(),
                }
                send_state(&mut stream, &mut cipher, ack);
                if matches!(case, "unavailable" | "unsupported_mode") {
                    send_frame(&mut stream, &mut cipher);
                }
                let _ = stream.read(&mut [0]);
            },
            false,
        );
        if matches!(case, "unavailable" | "unsupported_mode") {
            assert_eq!(result.codes, [2]);
            assert_eq!(result.video.frames.len(), 1);
        } else {
            assert!(result.video.frames.is_empty());
            assert!(
                result
                    .video
                    .events
                    .iter()
                    .any(|event| event["code"] == "INVALID_VIDEO_SETTINGS"),
                "{case}: {:?}",
                result.video.events
            );
        }
    }
}
