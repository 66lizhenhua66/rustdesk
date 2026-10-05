use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::BytesMut;
use protobuf::Message as _;
use remote_controller_core::{
    protos::rendezvous::{
        self as rv, IdPk, KeyExchange, KxParams, PunchHoleResponse, RelayResponse,
        RendezvousMessage,
    },
    session::*,
    upstream_crypto::{Encrypt, KxTranscript},
};
use serde_json::{json, Value};
use sodiumoxide::crypto::{box_, secretbox, sign};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
    time::Duration,
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
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut h = [0; 4];
    s.read_exact(&mut h[..1]).unwrap();
    let k = (h[0] & 3) as usize + 1;
    s.read_exact(&mut h[1..k]).unwrap();
    let mut b = vec![0; (u32::from_le_bytes(h) >> 2) as usize];
    s.read_exact(&mut b).unwrap();
    b
}
fn secure(s: &mut TcpStream, sk: &sign::SecretKey, modern: bool) -> Encrypt {
    let (pk, sec) = box_::gen_keypair();
    let mut wire_pk = pk.0;
    if modern {
        wire_pk[31] |= 0x80;
    }
    let mut m = RendezvousMessage::new();
    let version = u32::from(modern);
    let signed_params = if modern {
        let mut bytes = b"rdkx-params".to_vec();
        bytes.extend(
            KxParams {
                pk: wire_pk.to_vec(),
                version,
                ..Default::default()
            }
            .write_to_bytes()
            .unwrap(),
        );
        sign::sign(&bytes, sk)
    } else {
        vec![]
    };
    m.set_key_exchange(KeyExchange {
        keys: vec![sign::sign(&wire_pk, sk)],
        version,
        signed_params,
        ..Default::default()
    });
    send(s, &m.write_to_bytes().unwrap());
    let m = RendezvousMessage::parse_from_bytes(&recv(s)).unwrap();
    let Some(rv::rendezvous_message::Union::KeyExchange(k)) = m.union else {
        panic!()
    };
    let plain = box_::open(
        &k.keys[1],
        &box_::Nonce([0; 24]),
        &box_::PublicKey::from_slice(&k.keys[0]).unwrap(),
        &sec,
    )
    .unwrap();
    let key = secretbox::Key::from_slice(&plain).unwrap();
    if modern {
        Encrypt::new_split(
            key,
            false,
            &KxTranscript {
                initiator_pk: &k.keys[0],
                responder_pk: &wire_pk,
                advertised: 1,
                picked: 1,
            },
        )
        .unwrap()
    } else {
        Encrypt::new(key)
    }
}
fn read_enc(s: &mut TcpStream, c: &mut Encrypt) -> RendezvousMessage {
    let mut b = BytesMut::from(recv(s).as_slice());
    c.dec(&mut b).unwrap();
    RendezvousMessage::parse_from_bytes(&b).unwrap()
}
fn write_enc(s: &mut TcpStream, c: &mut Encrypt, m: RendezvousMessage) {
    let b = c.enc(&m.write_to_bytes().unwrap());
    send(s, &b)
}
fn addr(a: SocketAddr) -> Vec<u8> {
    let SocketAddr::V4(a) = a else { panic!() };
    let timestamp = 0x12345678u128;
    let n = ((u32::from_le_bytes(a.ip().octets()) as u128 + timestamp) << 49)
        | (timestamp << 17)
        | (a.port() as u128 + (timestamp & 0xffff));
    n.to_le_bytes().to_vec()
}
unsafe extern "C" fn collect(e: *const c_char, u: *mut c_void) {
    (&mut *(u as *mut Vec<Value>))
        .push(serde_json::from_str(CStr::from_ptr(e).to_str().unwrap()).unwrap());
}
#[derive(Clone, Copy)]
enum Case {
    Direct,
    Relay,
    Fallback,
    WrongServer,
    WrongPeer,
    WrongLivePeer,
    Modern,
    Timeout,
    Cancel,
    Oversize,
}
fn exercise(case: Case) -> Vec<Value> {
    sodiumoxide::init().unwrap();
    let (sp, ss) = sign::gen_keypair();
    let (pp, ps) = sign::gen_keypair();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = TcpListener::bind("127.0.0.1:0").unwrap();
    let relay = TcpListener::bind("127.0.0.1:0").unwrap();
    let sa = server.local_addr().unwrap();
    let pa = peer.local_addr().unwrap();
    let ra = relay.local_addr().unwrap();
    let req=CString::new(json!({"mode":if matches!(case,Case::Relay){"relay"}else{"id"},"server":if matches!(case,Case::Direct){format!("localhost:{}",sa.port())}else{sa.to_string()},"serverKey":STANDARD.encode(if matches!(case,Case::WrongServer){sign::gen_keypair().0.0}else{sp.0}),"relayServer":ra.to_string(),"peerId":"123456789","peerPublicKey":STANDARD.encode(pp.0)}).to_string()).unwrap();
    let task = controller_session_create(req.as_ptr(), CString::new("").unwrap().as_ptr(), 800);
    assert!(
        !task.is_null(),
        "ID session must accept configured transport without endpoint"
    );
    let fixture = thread::spawn(move || {
        let (mut s, _) = server.accept().unwrap();
        if matches!(case, Case::Oversize) {
            s.write_all(&((1024u32 * 1024) << 2 | 2).to_le_bytes()[..3])
                .unwrap();
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let _ = s.read(&mut [0; 1]);
            return;
        }
        if matches!(case, Case::Timeout | Case::Cancel) {
            thread::sleep(Duration::from_millis(1000));
            return;
        }
        if matches!(case, Case::WrongServer) {
            let (p, _) = box_::gen_keypair();
            let mut m = RendezvousMessage::new();
            m.set_key_exchange(KeyExchange {
                keys: vec![sign::sign(&p.0, &ss)],
                ..Default::default()
            });
            send(&mut s, &m.write_to_bytes().unwrap());
            return;
        }
        let mut c = secure(&mut s, &ss, matches!(case, Case::Modern));
        let q = read_enc(&mut s, &mut c);
        assert!(matches!(
            q.union,
            Some(rv::rendezvous_message::Union::PunchHoleRequest(_))
        ));
        let signed = sign::sign(
            &IdPk {
                id: "123456789".into(),
                pk: if matches!(case, Case::WrongPeer) {
                    vec![0; 32]
                } else {
                    pp.0.to_vec()
                },
                ..Default::default()
            }
            .write_to_bytes()
            .unwrap(),
            &ss,
        );
        let mut m = RendezvousMessage::new();
        if matches!(case, Case::Relay) {
            m.set_relay_response(RelayResponse {
                uuid: "test-uuid".into(),
                relay_server: ra.to_string(),
                union: Some(rv::relay_response::Union::Pk(signed)),
                ..Default::default()
            });
        } else {
            m.set_punch_hole_response(PunchHoleResponse {
                socket_addr: addr(pa),
                pk: signed,
                relay_server: ra.to_string(),
                ..Default::default()
            });
        }
        let peer = if matches!(case, Case::Fallback) {
            drop(peer);
            None
        } else {
            Some(peer)
        };
        write_enc(&mut s, &mut c, m);
        if matches!(case, Case::WrongPeer) {
            return;
        }
        if matches!(case, Case::Fallback) {
            let (mut s, _) = server.accept().unwrap();
            let mut c = secure(&mut s, &ss, matches!(case, Case::Modern));
            let q = read_enc(&mut s, &mut c);
            let Some(rv::rendezvous_message::Union::RequestRelay(r)) = q.union else {
                panic!()
            };
            let mut m = RendezvousMessage::new();
            m.set_relay_response(RelayResponse {
                uuid: r.uuid,
                relay_server: ra.to_string(),
                ..Default::default()
            });
            write_enc(&mut s, &mut c, m);
        }
        if matches!(case, Case::Relay | Case::Fallback) {
            let (mut r, _) = relay.accept().unwrap();
            let q = RendezvousMessage::parse_from_bytes(&recv(&mut r)).unwrap();
            assert!(matches!(
                q.union,
                Some(rv::rendezvous_message::Union::RequestRelay(_))
            ));
            login(&mut r, &ps);
        } else {
            let (mut p, _) = peer.unwrap().accept().unwrap();
            if matches!(case, Case::WrongLivePeer) {
                let mut m = remote_controller_core::protos::message::Message::new();
                m.set_signed_id(remote_controller_core::protos::message::SignedId {
                    id: vec![0; 64],
                    ..Default::default()
                });
                send(&mut p, &m.write_to_bytes().unwrap());
            } else {
                login(&mut p, &ps);
            }
        }
    });
    let canceller = if matches!(case, Case::Cancel) {
        let handle = task as usize;
        Some(thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            controller_session_cancel(handle as *mut ControllerSession);
        }))
    } else {
        None
    };
    let mut events = Vec::new();
    controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
    if let Some(canceller) = canceller {
        canceller.join().unwrap();
    }
    controller_session_destroy(task);
    fixture.join().unwrap();
    events
}
#[test]
fn coordinated_transport_requests_accept_missing_endpoint() {
    let e = exercise(Case::Timeout);
    assert_eq!(e.last().unwrap()["code"], "TIMEOUT");
}
#[test]
fn forced_relay_selects_relay_before_peer_authentication() {
    let e = exercise(Case::Relay);
    assert!(e.iter().any(|e| e["connectionPath"] == "relay"));
    assert!(e.iter().any(|e| e["code"] == "AUTHENTICATED"));
}
#[test]
fn wrong_server_signature_and_wrong_peer_binding_never_select_transport() {
    for c in [Case::WrongServer, Case::WrongPeer] {
        let e = exercise(c);
        assert!(!e.iter().any(|e| e["state"] == "transport_selected"));
        assert!(e.last().unwrap()["code"]
            .as_str()
            .unwrap()
            .contains("IDENTITY"));
    }
}
#[test]
fn direct_connect_failure_falls_back_to_relay() {
    let e = exercise(Case::Fallback);
    assert!(e.iter().any(|e| e["connectionPath"] == "relay"));
}
#[test]
fn cancellation_interrupts_coordination_read() {
    let e = exercise(Case::Cancel);
    assert_eq!(e.last().unwrap()["code"], "CANCELLED");
}

#[test]
fn coordinated_tcp_completes_strict_peer_login() {
    let e = exercise(Case::Direct);
    assert!(e.iter().any(|e| e["connectionPath"] == "id_direct"));
    assert!(e.iter().any(|e| e["code"] == "AUTHENTICATED"));
}
fn login(s: &mut TcpStream, sk: &sign::SecretKey) {
    use remote_controller_core::protos::message::{
        self as msg, login_response, Hash, LoginResponse, Message, PeerInfo, SignedId,
    };
    let (pk, secret) = box_::gen_keypair();
    let mut m = Message::new();
    m.set_signed_id(SignedId {
        id: sign::sign(
            &IdPk {
                id: "123456789".into(),
                pk: pk.0.to_vec(),
                kx_version: 1,
                ..Default::default()
            }
            .write_to_bytes()
            .unwrap(),
            sk,
        ),
        ..Default::default()
    });
    send(s, &m.write_to_bytes().unwrap());
    let Some(msg::message::Union::PublicKey(public)) =
        Message::parse_from_bytes(&recv(s)).unwrap().union
    else {
        panic!()
    };
    assert_eq!(public.kx_version, 1);
    let key = Encrypt::decode(&public.symmetric_value, &public.asymmetric_value, &secret).unwrap();
    let mut c = Encrypt::new_split(
        key,
        false,
        &KxTranscript {
            initiator_pk: &public.asymmetric_value,
            responder_pk: &pk.0,
            advertised: 1,
            picked: 1,
        },
    )
    .unwrap();
    let mut m = Message::new();
    m.set_hash(Hash {
        salt: "salt".into(),
        challenge: "fresh nonce".into(),
        ..Default::default()
    });
    send(s, &c.enc(&m.write_to_bytes().unwrap()));
    let mut b = BytesMut::from(recv(s).as_slice());
    c.dec(&mut b).unwrap();
    assert!(matches!(
        Message::parse_from_bytes(&b).unwrap().union,
        Some(msg::message::Union::LoginRequest(_))
    ));
    let mut m = Message::new();
    m.set_login_response(LoginResponse {
        union: Some(login_response::Union::PeerInfo(PeerInfo::new())),
        ..Default::default()
    });
    send(s, &c.enc(&m.write_to_bytes().unwrap()));
    let _ = recv(s);
}

#[test]
fn modern_server_signaling_completes_login() {
    let e = exercise(Case::Modern);
    assert!(e.iter().any(|e| e["code"] == "AUTHENTICATED"));
}
#[test]
fn coordinated_peer_signature_failure_never_falls_back() {
    let e = exercise(Case::WrongLivePeer);
    assert!(e.iter().any(|e| e["connectionPath"] == "id_direct"));
    assert!(!e.iter().any(|e| e["connectionPath"] == "relay"));
    assert_eq!(e.last().unwrap()["code"], "IDENTITY_INVALID");
}
#[test]
fn oversized_signaling_frame_is_rejected() {
    let e = exercise(Case::Oversize);
    assert_eq!(e.last().unwrap()["code"], "INVALID_FRAME");
}

#[test]
fn plaintext_unknown_or_unsigned_server_exchange_sends_no_request() {
    sodiumoxide::init().unwrap();
    for kind in 0..4 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (pk, sk) = sign::gen_keypair();
        let fixture = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut message = RendezvousMessage::new();
            if kind == 0 {
                message.set_punch_hole_response(PunchHoleResponse::new());
            } else {
                let (ephemeral, _) = box_::gen_keypair();
                let mut marked = ephemeral.0;
                if kind == 2 {
                    marked[31] |= 0x80;
                }
                message.set_key_exchange(KeyExchange {
                    keys: vec![sign::sign(&marked, &sk)],
                    version: if kind == 1 {
                        2
                    } else if kind == 3 {
                        1
                    } else {
                        0
                    },
                    ..Default::default()
                });
            }
            send(&mut socket, &message.write_to_bytes().unwrap());
            let mut unexpected = [0; 1];
            assert!(
                matches!(socket.read(&mut unexpected), Ok(0) | Err(_)),
                "must not send a request before authentication"
            );
        });
        let req = CString::new(json!({"mode":"id","server":address.to_string(),"serverKey":STANDARD.encode(pk.0),"relayServer":"127.0.0.1:1","peerId":"123456789","peerPublicKey":STANDARD.encode(pk.0)}).to_string()).unwrap();
        let task =
            controller_session_create(req.as_ptr(), CString::new("").unwrap().as_ptr(), 1000);
        assert!(!task.is_null());
        let mut events = Vec::<Value>::new();
        controller_session_run(task, Some(collect), &mut events as *mut _ as *mut c_void);
        controller_session_destroy(task);
        fixture.join().unwrap();
        assert_eq!(events.last().unwrap()["code"], "SERVER_IDENTITY_INVALID");
    }
}
#[test]
fn coordinated_profile_preserves_relay_and_requires_complete_pins_for_authentication() {
    use remote_controller_core::{controller_free_string, controller_validate_profile};
    fn validate(value: &Value) -> Value {
        let input = CString::new(value.to_string()).unwrap();
        let result = controller_validate_profile(input.as_ptr());
        let value =
            serde_json::from_str(unsafe { CStr::from_ptr(result) }.to_str().unwrap()).unwrap();
        controller_free_string(result);
        value
    }
    let key = STANDARD.encode([7; 32]);
    let mut profile = json!({"id":"p","name":"Server","mode":"id","target":"123456789","server":"hbbs.example.com","serverKey":key,"relayServer":"hbbr.example.com","peerId":"123456789","peerPublicKey":key,"peerFingerprint":""});
    let value = validate(&profile);
    assert_eq!(value["ok"], true);
    assert_eq!(value["profile"]["relayServer"], "hbbr.example.com:21117");
    assert_eq!(value["authenticationAvailable"], true);
    profile["relayServer"] = "".into();
    assert_eq!(validate(&profile)["authenticationAvailable"], false);
    profile["relayServer"] = "host:0".into();
    assert_eq!(validate(&profile)["code"], "INVALID_RELAY_SERVER");
    profile["relayServer"] = "hbbr.example.com".into();
    profile["peerId"] = "987654321".into();
    assert_eq!(validate(&profile)["code"], "PEER_ID_MISMATCH");
    profile["mode"] = "direct".into();
    profile["target"] = "127.0.0.1".into();
    profile["relayServer"] = "".into();
    assert_eq!(validate(&profile)["ok"], true);
}
