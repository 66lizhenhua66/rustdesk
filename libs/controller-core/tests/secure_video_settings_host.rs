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

use message_proto::{LoginRequest, Message, OptionMessage, OrdVideoSettings, SupportedDecoding};
use protobuf::Message as _;

fn settings(quality: &str, fps: u32, request_id: u64) -> OrdVideoSettings {
    OrdVideoSettings {
        version: 1,
        request_id,
        quality: quality.into(),
        fps,
        ..Default::default()
    }
}

fn login() -> LoginRequest {
    LoginRequest {
        username: "123456789".into(),
        my_id: "987654321".into(),
        ord_input_version: 2,
        option: Some(OptionMessage {
            supported_decoding: Some(SupportedDecoding {
                ability_vp8: 1,
                prefer: message_proto::supported_decoding::PreferCodec::VP8.into(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .into(),
        ..Default::default()
    }
}

#[test]
fn video_settings_are_only_accepted_as_explicit_control_opt_in() {
    let mut lr = login();
    assert!(policy::valid_login(&lr, "123456789"));
    lr.ord_video_settings = Some(settings("balanced", 15, 1)).into();
    assert!(policy::valid_login(&lr, "123456789"));
    lr.ord_video_settings.as_mut().unwrap().version = 2;
    assert!(!policy::valid_login(&lr, "123456789"));
    lr.ord_video_settings.as_mut().unwrap().version = 1;
    lr.ord_video_settings
        .as_mut()
        .unwrap()
        .special_fields
        .mut_unknown_fields()
        .add_varint(99, 1);
    assert!(!policy::valid_login(&lr, "123456789"));
    for invalid in [
        settings("ultra", 15, 1),
        settings("high", 60, 1),
        settings("high", 15, 0),
        settings("low", 10, 2),
    ] {
        lr.ord_video_settings = Some(invalid).into();
        assert!(!policy::valid_login(&lr, "123456789"));
    }
    lr.ord_video_settings = Some(settings("balanced", 15, 1)).into();
    lr.ord_input_version = 0;
    assert!(!policy::valid_login(&lr, "123456789"));
}

#[test]
fn runtime_settings_do_not_open_generic_options_or_reverse_state_messages() {
    let mut message = Message::new();
    for quality in ["low", "balanced", "high"] {
        message.set_ord_video_settings(settings(quality, 10, 2));
        assert!(message.compute_size() > sodiumoxide::crypto::secretbox::MACBYTES as u64);
    }
    message.set_ord_video_settings(settings("balanced", 15, 2));
    assert_eq!(
        policy::classify_message(&message),
        policy::Request::VideoSettings
    );
    let mut option = message_proto::Misc::new();
    option.set_option(OptionMessage::default());
    message.set_misc(option);
    assert_eq!(policy::classify_message(&message), policy::Request::Denied);
    message.set_ord_video_state(message_proto::OrdVideoState::default());
    assert_eq!(policy::classify_message(&message), policy::Request::Denied);
}

#[test]
fn approved_opt_in_updates_are_monotonic_bounded_and_acknowledge_actual_dimensions() {
    let mut lr = login();
    let update = settings("high", 30, 2);
    assert!(policy::updated_video_settings(&lr, true, 1, &update).is_none());
    lr.ord_video_settings = Some(settings("balanced", 15, 1)).into();
    assert!(policy::updated_video_settings(&lr, false, 1, &update).is_none());
    assert!(policy::updated_video_settings(&lr, true, 2, &update).is_none());
    let high = policy::updated_video_settings(&lr, true, 1, &update).unwrap();
    assert_eq!(high.dimensions(3840, 2160), Some((2560, 1440)));
    assert_eq!(high.dimensions(1025, 769), Some((1024, 768)));
    assert_eq!(high.dimensions(0, 1080), None);
    let balanced =
        policy::video_settings::Settings::parse(&settings("balanced", 15, 1), 0).unwrap();
    assert_eq!(balanced.dimensions(3840, 2160), Some((1920, 1080)));
    let low = policy::video_settings::Settings::parse(&settings("low", 10, 1), 0).unwrap();
    assert_eq!(low.dimensions(1920, 1200), Some((1152, 720)));
    let message_proto::message::Union::OrdVideoState(ack) = high.state(2560, 1440).union.unwrap()
    else {
        panic!("Expected video state");
    };
    assert_eq!(
        (
            ack.request_id,
            ack.quality.as_str(),
            ack.fps,
            ack.width,
            ack.height
        ),
        (2, "high", 30, 2560, 1440)
    );
    for (info, fields) in [
        (policy::approved_control_peer_info("test", 1280, 720), 5),
        (
            policy::approved_configurable_video_peer_info("test", 1920, 1080),
            6,
        ),
    ] {
        let message_proto::message::Union::LoginResponse(login) = info.union.unwrap() else {
            panic!("Expected login");
        };
        let message_proto::login_response::Union::PeerInfo(peer) = login.union.unwrap() else {
            panic!("Expected peer info");
        };
        let metadata: serde_json::Value = serde_json::from_str(&peer.platform_additions).unwrap();
        assert_eq!(metadata.as_object().unwrap().len(), fields);
        if fields == 6 {
            assert_eq!(metadata["video_settings_version"], 1);
        } else {
            assert!(metadata.get("video_settings_version").is_none());
        }
    }
    let mut unknown = update.clone();
    unknown
        .special_fields
        .mut_unknown_fields()
        .add_varint(99, 1);
    assert!(policy::updated_video_settings(&lr, true, 1, &unknown).is_none());
    lr.ord_access = Some(message_proto::OrdAccessRequest::default()).into();
    assert!(!policy::valid_login(&lr, "123456789"));
}
