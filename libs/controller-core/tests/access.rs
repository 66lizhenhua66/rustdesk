use base64::{engine::general_purpose::STANDARD, Engine};
use remote_controller_core::{
    access::{controller_identity_create_v1, controller_secret_string_free_v1},
    session::{controller_connection_create_access_v1, controller_session_destroy},
};
use serde_json::json;
use std::ffi::{CStr, CString};
#[test]
fn identity_seed_matches_public_key_and_access_rejects_non_video_mode() {
    let raw = controller_identity_create_v1();
    assert!(!raw.is_null());
    let identity: serde_json::Value =
        serde_json::from_str(unsafe { CStr::from_ptr(raw) }.to_str().unwrap()).unwrap();
    controller_secret_string_free_v1(raw);
    let seed = STANDARD.decode(identity["seed"].as_str().unwrap()).unwrap();
    let (pk, _) = sodiumoxide::crypto::sign::keypair_from_seed(
        &sodiumoxide::crypto::sign::Seed::from_slice(&seed).unwrap(),
    );
    assert_eq!(STANDARD.encode(pk.0), identity["publicKey"]);
    let secrets = CString::new(
        json!({"mode":"pair","seed":identity["seed"],"credentialId":"","credentialSecret":""})
            .to_string(),
    )
    .unwrap();
    for mode in ["secure_control", "secure_host", "demo", "secure_video"] {
        let request=CString::new(json!({"endpoint":"127.0.0.1:1","peerId":"123456789","peerPublicKey":identity["publicKey"],"expectedPeer":mode}).to_string()).unwrap();
        let task = controller_connection_create_access_v1(request.as_ptr(), secrets.as_ptr(), 1000);
        assert_eq!(task.is_null(), mode != "secure_video");
        if !task.is_null() {
            controller_session_destroy(task);
        }
    }
}
use bytes::BytesMut;
use protobuf::Message as _;
use remote_controller_core::{
    access_proof::access_proof_payload,
    protos::{
        message::{
            self as msg, login_response, DisplayInfo, Hash, LoginResponse, Message, Misc,
            OrdAccessGrant, PeerInfo, SignedId,
        },
        rendezvous::IdPk,
    },
    session::{controller_session_run_video, ControllerVideoCallback},
    upstream_crypto::{Encrypt, KxTranscript},
};
use sodiumoxide::crypto::{box_, sign};
use std::{
    ffi::{c_char, c_void},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn send(s: &mut TcpStream, b: &[u8]) {
    let n = b.len() as u32;
    let (h, k) = if n < 64 {
        (n << 2, 1)
    } else if n < 16384 {
        (n << 2 | 1, 2)
    } else {
        (n << 2 | 2, 3)
    };
    s.write_all(&h.to_le_bytes()[..k]).unwrap();
    s.write_all(b).unwrap();
}
fn recv(s: &mut TcpStream) -> Vec<u8> {
    let mut h = [0; 4];
    s.read_exact(&mut h[..1]).unwrap();
    let k = (h[0] & 3) as usize + 1;
    s.read_exact(&mut h[1..k]).unwrap();
    let mut b = vec![0; (u32::from_le_bytes(h) >> 2) as usize];
    s.read_exact(&mut b).unwrap();
    b
}
unsafe extern "C" fn collect(e: *const c_char, u: *mut c_void) {
    (&mut *(u as *mut Vec<serde_json::Value>))
        .push(serde_json::from_str(CStr::from_ptr(e).to_str().unwrap()).unwrap());
}
unsafe extern "C" fn video(_: *const u8, _: u32, _: u32, _: u32, _: i64, _: u8, _: *mut c_void) {}
fn exercise(mode: &str, case: &str) -> Vec<serde_json::Value> {
    sodiumoxide::init().unwrap();
    let (pk, sk) = sign::gen_keypair();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mode = mode.to_owned();
    let server_mode = mode.clone();
    let case = case.to_owned();
    let fixture = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let (ephemeral, secret) = box_::gen_keypair();
        let mut m = Message::new();
        m.set_signed_id(SignedId {
            id: sign::sign(
                &IdPk {
                    id: "123456789".into(),
                    pk: ephemeral.0.to_vec(),
                    kx_version: 1,
                    ..Default::default()
                }
                .write_to_bytes()
                .unwrap(),
                &sk,
            ),
            ..Default::default()
        });
        send(&mut s, &m.write_to_bytes().unwrap());
        let Some(msg::message::Union::PublicKey(public)) =
            Message::parse_from_bytes(&recv(&mut s)).unwrap().union
        else {
            panic!()
        };
        let key =
            Encrypt::decode(&public.symmetric_value, &public.asymmetric_value, &secret).unwrap();
        let mut cipher = Encrypt::new_split(
            key,
            false,
            &KxTranscript {
                initiator_pk: &public.asymmetric_value,
                responder_pk: &ephemeral.0,
                advertised: 1,
                picked: 1,
            },
        )
        .unwrap();
        let challenge = STANDARD.encode([42; 32]);
        let mut m = Message::new();
        m.set_hash(Hash {
            salt: "salt".into(),
            challenge: challenge.clone(),
            ..Default::default()
        });
        send(&mut s, &cipher.enc(&m.write_to_bytes().unwrap()));
        let mut bytes = BytesMut::from(recv(&mut s).as_slice());
        cipher.dec(&mut bytes).unwrap();
        let Some(msg::message::Union::LoginRequest(login)) =
            Message::parse_from_bytes(&bytes).unwrap().union
        else {
            panic!()
        };
        let access = login
            .ord_access
            .as_ref()
            .expect("access proof must be sent");
        assert_eq!(access.mode, server_mode);
        assert!(login.password.is_empty());
        let payload = access_proof_payload("123456789", &pk.0, &challenge, access).unwrap();
        let controller = sign::PublicKey::from_slice(&access.controller_public_key).unwrap();
        assert!(sign::verify_detached(
            &sign::Signature::from_bytes(&access.proof).unwrap(),
            &payload,
            &controller
        ));
        for (target, key, challenge) in [
            ("987654321", pk.0, challenge.clone()),
            ("123456789", [0; 32], challenge.clone()),
            ("123456789", pk.0, STANDARD.encode([43; 32])),
        ] {
            let wrong = access_proof_payload(target, &key, &challenge, access).unwrap();
            assert!(!sign::verify_detached(
                &sign::Signature::from_bytes(&access.proof).unwrap(),
                &wrong,
                &controller
            ));
        }
        let mut unknown = access.clone();
        unknown
            .special_fields
            .mut_unknown_fields()
            .add_varint(99, 1);
        assert!(access_proof_payload("123456789", &pk.0, &challenge, &unknown).is_err());
        let mut wrong = access.clone();
        wrong.scope = "windows_primary".into();
        assert!(access_proof_payload("123456789", &pk.0, &challenge, &wrong).is_err());
        if server_mode == "unattended" {
            assert_eq!(access.credential_id, vec![7; 16]);
            assert_eq!(access.credential_secret, vec![8; 32]);
            wrong = access.clone();
            wrong.credential_secret[0] ^= 1;
            let payload = access_proof_payload("123456789", &pk.0, &challenge, &wrong).unwrap();
            assert!(!sign::verify_detached(
                &sign::Signature::from_bytes(&access.proof).unwrap(),
                &payload,
                &controller
            ));
        }
        if case == "2fa" || case == "rejected" {
            let mut m = Message::new();
            m.set_login_response(LoginResponse {
                union: Some(login_response::Union::Error(
                    if case == "2fa" {
                        "2FA Required"
                    } else {
                        "Access rejected"
                    }
                    .into(),
                )),
                ..Default::default()
            });
            send(&mut s, &cipher.enc(&m.write_to_bytes().unwrap()));
            let _ = s.read(&mut [0; 1]);
            return;
        }
        if (server_mode == "pair" && case != "missing") || case == "unexpected" {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            let mut grant = OrdAccessGrant {
                version: 1,
                credential_id: vec![7; 16],
                credential_secret: vec![8; 32],
                controller_public_key: controller.0.to_vec(),
                expires_at_ms: now + 60000,
                scope: "windows_primary_view".into(),
                ..Default::default()
            };
            if case == "scope" {
                grant.scope = "windows_primary".into();
            }
            if case == "key" {
                grant.controller_public_key = vec![0; 32];
            }
            if case == "expired" {
                grant.expires_at_ms = now - 1;
            }
            let mut m = Message::new();
            m.set_ord_access_grant(grant);
            send(&mut s, &cipher.enc(&m.write_to_bytes().unwrap()));
            if case == "duplicate" {
                send(&mut s, &cipher.enc(&m.write_to_bytes().unwrap()));
            }
            if ["scope", "key", "expired", "duplicate", "unexpected"].contains(&case.as_str()) {
                let _ = s.read(&mut [0; 1]);
                return;
            }
        }
        let mut additions =
            json!({"ord_secure_host":1,"media":true,"video_codec":"vp8","input_scope":"none"});
        if case != "legacy" {
            additions["access_mode"] = server_mode.into();
        }
        let mut m = Message::new();
        m.set_login_response(LoginResponse {
            union: Some(login_response::Union::PeerInfo(PeerInfo {
                platform_additions: additions.to_string(),
                displays: vec![DisplayInfo {
                    width: 640,
                    height: 360,
                    ..Default::default()
                }],
                ..Default::default()
            })),
            ..Default::default()
        });
        send(&mut s, &cipher.enc(&m.write_to_bytes().unwrap()));
        let mut m = Message::new();
        let mut misc = Misc::new();
        misc.set_close_reason("done".into());
        m.set_misc(misc);
        send(&mut s, &cipher.enc(&m.write_to_bytes().unwrap()));
        let _ = s.read(&mut [0; 1]);
    });
    let request=CString::new(json!({"endpoint":addr.to_string(),"peerId":"123456789","peerPublicKey":STANDARD.encode(pk.0),"expectedPeer":"secure_video"}).to_string()).unwrap();
    let creds=CString::new(json!({"mode":mode,"seed":STANDARD.encode([1;32]),"credentialId":if mode=="pair"{String::new()}else{STANDARD.encode([7;16])},"credentialSecret":if mode=="pair"{String::new()}else{STANDARD.encode([8;32])}}).to_string()).unwrap();
    let task = controller_connection_create_access_v1(request.as_ptr(), creds.as_ptr(), 2000);
    assert!(!task.is_null());
    let mut events = Vec::new();
    controller_session_run_video(
        task,
        Some(collect),
        Some(video as ControllerVideoCallback),
        &mut events as *mut _ as *mut c_void,
    );
    controller_session_destroy(task);
    fixture.join().unwrap();
    events
}
#[test]
fn pairing_and_unattended_verify_fresh_proof_and_exact_access_mode() {
    for mode in ["pair", "unattended"] {
        let e = exercise(mode, "ok");
        assert!(e
            .iter()
            .any(|e| e["state"] == "connected" && e["accessMode"] == mode));
        assert_eq!(
            e.iter().filter(|e| e["state"] == "access_paired").count(),
            usize::from(mode == "pair")
        );
    }
}
#[test]
fn pairing_rejects_missing_duplicate_or_invalid_grant() {
    for case in ["missing", "duplicate", "scope", "key", "expired"] {
        let e = exercise("pair", case);
        assert!(!e.iter().any(|e| e["state"] == "connected"), "{case}");
        assert!(e.last().unwrap()["code"]
            .as_str()
            .unwrap()
            .starts_with("ACCESS_"));
    }
}
#[test]
fn trusted_access_rejects_legacy_peerinfo_unsolicited_grant_and_second_factor() {
    for (mode, case) in [
        ("pair", "legacy"),
        ("unattended", "legacy"),
        ("unattended", "unexpected"),
        ("pair", "2fa"),
        ("unattended", "rejected"),
    ] {
        let e = exercise(mode, case);
        assert!(!e.iter().any(|e| e["state"] == "connected"));
    }
}
