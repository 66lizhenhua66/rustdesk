extern crate self as base;
extern crate self as hbb_common;
pub use anyhow;
pub use anyhow::{bail, Result as ResultType};
pub use base64;
pub use protobuf;
pub use remote_controller_core::protos::message as message_proto;
pub use sodiumoxide;
pub use remote_controller_core::upstream_crypto as tcp;
pub use remote_controller_core::protos::rendezvous as rendezvous_proto;
#[path = "../../../src/server/secure_host_policy.rs"]
mod host_policy;
#[path = "../../../src/server/secure_access_policy.rs"]
mod access;
#[path = "../../base/src/access_proof.rs"]
pub mod access_proof;
use sodiumoxide::crypto::sign;

#[test]
fn durable_grants_validate_reload_revoke_and_fail_on_corruption() {
    sodiumoxide::init().unwrap();
    let (pk, sk) = sign::gen_keypair();
    let ctx = access::Context {
        target: "123456789".into(),
        public: pk.0.to_vec(),
        policy: "policy-a".into(),
        enabled: true,
        two_factor: false,
    };
    let path = std::env::temp_dir().join(format!("ord-access-test-{}.json", std::process::id()));
    let controller = sign::gen_keypair().0;
    let (grant, record) =
        access::pair(&path, &ctx, &sk.0, &controller.0, "controller", 1000).unwrap();
    assert!(access::authorize(
        &path,
        &ctx,
        &controller.0,
        &grant.credential_id,
        &grant.credential_secret,
        2000
    )
    .is_ok());
    assert!(access::authorize(
        &path,
        &ctx,
        &controller.0,
        &grant.credential_id,
        &[7; 32],
        2000
    )
    .is_err());
    let mut changed = ctx.clone();
    changed.policy = "policy-b".into();
    assert!(access::active(&path, &changed, &record.id, 2000).is_err());
    assert!(access::active(&path, &ctx, &record.id, record.expires_at_ms).is_err());
    access::revoke(&path, &ctx, Some(&record.id)).unwrap();
    assert!(access::active(&path, &ctx, &record.id, 2000).is_err());
    std::fs::write(&path, "broken").unwrap();
    assert!(access::list(&path, &ctx, 2000).is_err());
    access::revoke(&path, &ctx, None).unwrap();
    assert!(access::list(&path, &ctx, 2000).unwrap().is_empty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn disabled_two_factor_and_unapproved_pair_cannot_issue_grants() {
    sodiumoxide::init().unwrap();
    let (pk, sk) = sign::gen_keypair();
    let path =
        std::env::temp_dir().join(format!("ord-access-disabled-{}.json", std::process::id()));
    let mut ctx = access::Context {
        target: "123456789".into(),
        public: pk.0.to_vec(),
        policy: "a".into(),
        enabled: false,
        two_factor: false,
    };
    assert!(access::pair(&path, &ctx, &sk.0, &pk.0, "x", 1000).is_err());
    ctx.enabled = true;
    ctx.two_factor = true;
    assert!(access::pair(&path, &ctx, &sk.0, &pk.0, "x", 1000).is_err());
    assert!(!access::pair_approval_allowed(true, false, false, true));
    assert!(!access::pair_approval_allowed(true, true, true, true));
    assert!(access::pair_approval_allowed(true, false, true, true));
}

#[test]
fn possession_proof_binds_target_scope_and_fresh_connection() {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    sodiumoxide::init().unwrap();
    let (host, _) = sign::gen_keypair();
    let (controller, secret) = sign::gen_keypair();
    let ctx = access::Context {
        target: "123456789".into(),
        public: host.0.to_vec(),
        policy: "policy".into(),
        enabled: true,
        two_factor: false,
    };
    let challenge = STANDARD.encode([8; 32]);
    let mut request = message_proto::OrdAccessRequest {
        version: 1,
        mode: "pair".into(),
        controller_public_key: controller.0.to_vec().into(),
        scope: "windows_primary_view".into(),
        proof: vec![0; 64].into(),
        ..Default::default()
    };
    let payload =
        access_proof::access_proof_payload(&ctx.target, &ctx.public, &challenge, &request).unwrap();
    request.proof = sign::sign_detached(&payload, &secret)
        .as_ref()
        .to_vec()
        .into();
    assert!(access::verify_request(&ctx, &challenge, &request).is_ok());
    assert!(access::verify_request(&ctx, &STANDARD.encode([9; 32]), &request).is_err());
    let mut other = ctx.clone();
    other.target = "987654321".into();
    assert!(access::verify_request(&other, &challenge, &request).is_err());
    request.scope = "windows_primary".into();
    assert!(access::verify_request(&ctx, &challenge, &request).is_err());
    request.scope = "windows_primary_view".into();
    request.controller_public_key = sign::gen_keypair().0 .0.to_vec().into();
    assert!(access::verify_request(&ctx, &challenge, &request).is_err());
}

#[test]
fn access_login_rejects_unknown_outer_protocol_fields() {
    let mut msg = message_proto::Message::new();
    let mut lr=message_proto::LoginRequest::new();
    lr.ord_access=Some(message_proto::OrdAccessRequest::new()).into();
    msg.set_login_request(lr);
    assert_eq!(host_policy::classify_message(&msg), host_policy::Request::Login);
    msg.special_fields.mut_unknown_fields().add_varint(99,1);
    assert_eq!(host_policy::classify_message(&msg), host_policy::Request::Denied);
}