use protobuf::Message as _;
use remote_controller_core::{protos::rendezvous::IdPk, upstream_crypto};
use sodiumoxide::crypto::{box_, sign};

#[test]
fn signed_identity_is_verified_and_tampering_is_rejected() {
    sodiumoxide::init().unwrap();
    let (public, secret) = sign::gen_keypair();
    let identity = IdPk {
        id: "123456789".into(),
        pk: vec![42; 32],
        kx_version: 1,
        ..Default::default()
    };
    let mut signed = sign::sign(&identity.write_to_bytes().unwrap(), &secret);
    let (id, ephemeral, _, version) = upstream_crypto::decode_id_pk_dtls(&signed, &public).unwrap();
    assert_eq!(id, "123456789");
    assert_eq!(ephemeral, [42; 32]);
    assert_eq!(version, 1);
    let (other_public, _) = sign::gen_keypair();
    assert!(upstream_crypto::decode_id_pk_dtls(&signed, &other_public).is_err());
    signed[0] ^= 1;
    assert!(upstream_crypto::decode_id_pk_dtls(&signed, &public).is_err());
}

#[test]
fn signed_identity_with_wrong_ephemeral_key_length_is_rejected() {
    sodiumoxide::init().unwrap();
    let (public, secret) = sign::gen_keypair();
    let identity = IdPk {
        id: "123456789".into(),
        pk: vec![42; 31],
        ..Default::default()
    };
    let signed = sign::sign(&identity.write_to_bytes().unwrap(), &secret);
    assert!(upstream_crypto::decode_id_pk_dtls(&signed, &public).is_err());
}

#[test]
fn upstream_sealed_session_key_round_trips() {
    sodiumoxide::init().unwrap();
    let (peer_public, peer_secret) = box_::gen_keypair();
    let (controller_public, envelope, session_key) =
        upstream_crypto::create_symmetric_key_msg(peer_public.0);
    let opened =
        upstream_crypto::Encrypt::decode(&envelope, &controller_public, &peer_secret).unwrap();
    assert_eq!(opened.0, session_key.0);
    let mut tampered = envelope.to_vec();
    tampered[0] ^= 1;
    assert!(upstream_crypto::Encrypt::decode(&tampered, &controller_public, &peer_secret).is_err());
}
