extern crate self as base;
extern crate self as hbb_common;

pub use anyhow::{bail, Result as ResultType};
pub use base64;
pub use protobuf;
pub use remote_controller_core::{
    protos::{message as message_proto, rendezvous as rendezvous_proto},
    upstream_crypto as tcp,
};
pub use sodiumoxide;

#[path = "../../../src/server/secure_host_policy.rs"]
mod policy;

use message_proto::{message, misc, LoginRequest, Message, MouseEvent, PublicKey};
use sodiumoxide::crypto::sign;

#[test]
fn listener_is_explicit_and_lan_requires_concrete_sources() {
    assert!(policy::parse_listen("", "").unwrap().is_none());
    for endpoint in ["0.0.0.0:21120", "[::]:21120", "127.0.0.1:0", "host:21120"] {
        assert!(policy::parse_listen(endpoint, "").is_err());
    }
    assert!(policy::parse_listen("192.168.1.10:21120", "").is_err());
    assert!(policy::parse_listen("192.168.1.10:21120", "0.0.0.0").is_err());
    let lan = policy::parse_listen("192.168.1.10:21120", "192.168.1.20")
        .unwrap()
        .unwrap();
    assert!(lan.permits("192.168.1.20".parse().unwrap()));
    assert!(!lan.permits("192.168.1.21".parse().unwrap()));
    let local = policy::parse_listen("127.0.0.1:21120", "")
        .unwrap()
        .unwrap();
    assert_eq!(local.address.port(), 21120);
    assert!(local.permits("127.0.0.1".parse().unwrap()));
    assert!(!local.permits("192.168.1.20".parse().unwrap()));
}

fn handshake() -> (policy::IdentityHandshake, sign::PublicKey) {
    sodiumoxide::init().unwrap();
    let (pk, sk) = sign::gen_keypair();
    (
        policy::IdentityHandshake::new("123456789", &sk.0, &pk.0).unwrap(),
        pk,
    )
}

#[test]
fn official_identity_binds_v1_keys_and_encrypts_both_directions() {
    let (handshake, pk) = handshake();
    let Some(message::Union::SignedId(signed)) = &handshake.message.union else {
        panic!()
    };
    let (id, ephemeral, _, version) = tcp::decode_id_pk_dtls(&signed.id, &pk).unwrap();
    assert_eq!(id, "123456789");
    assert_eq!(version, 1);
    assert_eq!(ephemeral, handshake.public);
    let (client_pk, envelope, client_key) = tcp::create_symmetric_key_msg(ephemeral);
    let mut reply = Message::new();
    reply.set_public_key(PublicKey {
        asymmetric_value: client_pk.clone().into(),
        symmetric_value: envelope.into(),
        kx_version: 1,
        ..Default::default()
    });
    let (server_key, actual_client_pk) = handshake.finish(&reply).unwrap();
    assert_eq!(actual_client_pk.as_slice(), &client_pk[..]);
    let transcript = tcp::KxTranscript {
        initiator_pk: &actual_client_pk,
        responder_pk: &handshake.public,
        advertised: 1,
        picked: 1,
    };
    let mut client = tcp::Encrypt::new_split(client_key, true, &transcript).unwrap();
    let mut server = tcp::Encrypt::new_split(server_key, false, &transcript).unwrap();
    let mut bytes = bytes::BytesMut::from(client.enc(b"login").as_slice());
    server.dec(&mut bytes).unwrap();
    assert_eq!(&bytes[..], b"login");
    let mut bytes = bytes::BytesMut::from(server.enc(b"approved").as_slice());
    client.dec(&mut bytes).unwrap();
    assert_eq!(&bytes[..], b"approved");
}

#[test]
fn missing_identity_empty_key_legacy_and_tampered_envelopes_fail_closed() {
    let (handshake, _) = handshake();
    assert!(policy::IdentityHandshake::new("123456789", &[], &[]).is_err());
    assert!(handshake.finish(&Message::new()).is_err());
    let (client_pk, envelope, _) = tcp::create_symmetric_key_msg(handshake.public);
    for version in [0, 2, u32::MAX] {
        let mut reply = Message::new();
        reply.set_public_key(PublicKey {
            asymmetric_value: client_pk.clone().into(),
            symmetric_value: envelope.clone().into(),
            kx_version: version,
            ..Default::default()
        });
        assert!(handshake.finish(&reply).is_err());
    }
    let mut reply = Message::new();
    reply.set_public_key(PublicKey {
        kx_version: 1,
        ..Default::default()
    });
    assert!(handshake.finish(&reply).is_err());
    let mut tampered = envelope.to_vec();
    tampered[0] ^= 1;
    reply.set_public_key(PublicKey {
        asymmetric_value: client_pk.into(),
        symmetric_value: tampered.into(),
        kx_version: 1,
        ..Default::default()
    });
    assert!(handshake.finish(&reply).is_err());
}

#[test]
fn nonmedia_entry_rejects_login_extensions_and_all_operations() {
    let mut login = LoginRequest {
        username: "123456789".into(),
        my_id: "987654321".into(),
        my_name: "Controller".into(),
        ..Default::default()
    };
    assert!(policy::valid_login(&login, "123456789"));
    login.option = Some(message_proto::OptionMessage {
        disable_keyboard: message_proto::option_message::BoolOption::No.into(),
        disable_clipboard: message_proto::option_message::BoolOption::No.into(),
        ..Default::default()
    })
    .into();
    // Metadata is accepted for existing controllers, but never applied by the strict branch.
    assert!(policy::valid_login(&login, "123456789"));
    login.password = vec![1; 32].into();
    assert!(!policy::valid_login(&login, "123456789"));
    login.password.clear();
    login.set_file_transfer(Default::default());
    assert!(!policy::valid_login(&login, "123456789"));
    login.union = None;
    login.os_login = Some(message_proto::OSLogin {
        username: "admin".into(),
        ..Default::default()
    })
    .into();
    assert!(!policy::valid_login(&login, "123456789"));
    for message in [
        Message::new(),
        {
            let mut m = Message::new();
            m.set_mouse_event(MouseEvent::new());
            m
        },
        {
            let mut m = Message::new();
            m.set_switch_sides_response(Default::default());
            m
        },
    ] {
        assert_eq!(policy::classify_message(&message), policy::Request::Denied);
    }
    let mut msg = Message::new();
    msg.set_test_delay(Default::default());
    assert_eq!(policy::classify_message(&msg), policy::Request::Heartbeat);
}

#[test]
fn approval_advertises_no_media_or_input_and_public_export_has_no_secret() {
    let snapshot = policy::permission_snapshot();
    assert_eq!(snapshot.len(), 8);
    for message in snapshot {
        let Some(message::Union::Misc(m)) = message.union else {
            panic!()
        };
        let Some(misc::Union::PermissionInfo(p)) = m.union else {
            panic!()
        };
        assert!(!p.enabled);
    }
    let message = policy::approved_peer_info("1.5.0");
    let additions: serde_json::Value =
        serde_json::from_str(&message.login_response().peer_info().platform_additions).unwrap();
    assert_eq!(additions["ord_secure_host"], 1);
    assert_eq!(additions["media"], false);
    assert_eq!(additions["input_scope"], "none");
    assert!(additions.get("ord_demo").is_none());
    let (_, pk) = handshake();
    let exported =
        policy::public_profile("123456789", &pk.0, "127.0.0.1:21120".parse().unwrap()).unwrap();
    let result: serde_json::Value = serde_json::from_str(&exported).unwrap();
    assert_eq!(result["peerId"], "123456789");
    assert!(result.get("password").is_none());
    assert!(result.get("secretKey").is_none());
    let input = std::ffi::CString::new(exported).unwrap();
    let output = remote_controller_core::controller_validate_profile(input.as_ptr());
    let parsed: serde_json::Value = serde_json::from_str(
        unsafe { std::ffi::CStr::from_ptr(output) }
            .to_str()
            .unwrap(),
    )
    .unwrap();
    remote_controller_core::controller_free_string(output);
    assert_eq!(parsed["ok"], true);
}
