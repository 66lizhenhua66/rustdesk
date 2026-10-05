extern crate self as hbb_common;
pub use anyhow::{bail, Result as ResultType};
pub use base64;
pub use protobuf;
pub use remote_controller_core::protos::rendezvous as rendezvous_proto;
#[path = "../../../src/server/secure_rendezvous_policy.rs"]
mod host;
#[test]
fn explicit_complete_configuration_only() {
    let key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    assert!(host::RendezvousConfig::parse("", "", "", "")
        .unwrap()
        .is_none());
    for values in [
        ("hbbs:21116", "", "hbbr:21117", "127.0.0.1"),
        ("hbbs:21116", key, "hbbr:21117", ""),
        ("hbbs", key, "hbbr:21117", "127.0.0.1"),
    ] {
        assert!(host::RendezvousConfig::parse(values.0, values.1, values.2, values.3).is_err());
    }
    assert!(
        host::RendezvousConfig::parse("hbbs:21116", key, "hbbr:21117", "127.0.0.1")
            .unwrap()
            .is_some()
    );
}
#[test]
fn original_source_and_fixed_relay_are_required() {
    let cfg = host::RendezvousConfig::parse(
        "hbbs:21116",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "hbbr:21117",
        "192.168.1.20",
    )
    .unwrap()
    .unwrap();
    assert!(cfg
        .validate_request("192.168.1.20:2000".parse().unwrap(), "hbbr:21117")
        .is_ok());
    assert!(cfg
        .validate_request("192.168.1.21:2000".parse().unwrap(), "hbbr:21117")
        .is_err());
    assert!(cfg
        .validate_request("192.168.1.20:2000".parse().unwrap(), "evil:21117")
        .is_err());
    assert!(host::decode_source(&[255; 17]).is_err());
    assert!(host::decode_source(&[1]).is_err());
}

#[test]
fn hbbs_registration_accepts_only_success_and_source_roundtrips_wire_address() {
    use hbb_common::rendezvous_proto::{
        register_pk_response, RegisterPkResponse, RendezvousMessage,
    };
    use protobuf::Message as _;
    let mut message = RendezvousMessage::new();
    message.set_register_pk_response(RegisterPkResponse {
        result: register_pk_response::Result::UUID_MISMATCH.into(),
        ..Default::default()
    });
    assert!(host::validate_registration(&message.write_to_bytes().unwrap()).is_err());
    message.set_register_pk_response(RegisterPkResponse::default());
    assert!(host::validate_registration(&message.write_to_bytes().unwrap()).is_ok());
    assert!(host::validate_registration(&[]).is_err());
    let tm = 100_u128;
    let ip = u32::from_le_bytes([192, 168, 1, 20]) as u128;
    let encoded = (((ip + tm) << 49) | (tm << 17) | (21118 + (tm & 0xffff))).to_le_bytes();
    assert_eq!(
        host::decode_source(&encoded).unwrap(),
        "192.168.1.20:21118".parse().unwrap()
    );
}
