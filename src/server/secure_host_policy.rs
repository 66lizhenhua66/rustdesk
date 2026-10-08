use std::net::{IpAddr, SocketAddr};

use base::message_proto::{
    message, misc, permission_info::Permission, supported_decoding::PreferCodec, DisplayInfo,
    LoginRequest, LoginResponse, Message, Misc, PeerInfo, PermissionInfo, SignedId,
};
use hbb_common::{
    bail,
    base64::{engine::general_purpose::STANDARD, Engine as _},
    protobuf::Message as _,
    rendezvous_proto::IdPk,
    sodiumoxide::{
        self,
        crypto::{box_, secretbox, sign},
    },
    tcp::Encrypt,
    ResultType,
};
use sha2::{Digest, Sha256};

#[path = "secure_video_settings.rs"]
pub mod video_settings;

pub struct ListenConfig {
    pub address: SocketAddr,
    pub allowed: Vec<IpAddr>,
}

impl ListenConfig {
    pub fn permits(&self, source: IpAddr) -> bool {
        self.allowed.contains(&source)
    }
}

pub fn parse_listen(endpoint: &str, sources: &str) -> ResultType<Option<ListenConfig>> {
    if endpoint.is_empty() {
        return Ok(None);
    }
    let address: SocketAddr = endpoint.parse()?;
    if address.ip().is_unspecified() || address.ip().is_multicast() || address.port() == 0 {
        bail!("Secure host needs a concrete unicast address and nonzero port");
    }
    let allowed = if sources.is_empty() && address.ip().is_loopback() {
        vec![address.ip()]
    } else {
        let mut allowed = Vec::new();
        for source in sources.split(',') {
            let ip: IpAddr = source.trim().parse()?;
            if ip.is_unspecified() || ip.is_multicast() || allowed.len() >= 16 {
                bail!("Secure host needs at most 16 concrete source IPs");
            }
            allowed.push(ip);
        }
        allowed
    };
    Ok(Some(ListenConfig { address, allowed }))
}

pub struct IdentityHandshake {
    pub message: Message,
    pub public: [u8; box_::PUBLICKEYBYTES],
    secret: box_::SecretKey,
}

impl IdentityHandshake {
    pub fn new(id: &str, sk: &[u8], pk: &[u8]) -> ResultType<Self> {
        if sodiumoxide::init().is_err() {
            bail!("Crypto initialization failed");
        }
        if !(6..=20).contains(&id.len()) || !id.bytes().all(|c| c.is_ascii_digit()) {
            bail!("Secure host requires a valid peer ID");
        }
        let Some(secret) = sign::SecretKey::from_slice(sk) else {
            bail!("Secure host requires a signing secret");
        };
        let Some(public) = sign::PublicKey::from_slice(pk) else {
            bail!("Secure host requires a signing public key");
        };
        let proof = b"Open Remote Desk identity key consistency";
        if !sign::verify_detached(&sign::sign_detached(proof, &secret), proof, &public) {
            bail!("Secure host signing key pair does not match");
        }
        let (ephemeral_public, ephemeral_secret) = box_::gen_keypair();
        let identity = IdPk {
            id: id.to_owned(),
            pk: ephemeral_public.0.to_vec().into(),
            kx_version: 1,
            ..Default::default()
        };
        let mut message = Message::new();
        message.set_signed_id(SignedId {
            id: sign::sign(&identity.write_to_bytes()?, &secret).into(),
            ..Default::default()
        });
        Ok(Self {
            message,
            public: ephemeral_public.0,
            secret: ephemeral_secret,
        })
    }

    pub fn finish(
        &self,
        reply: &Message,
    ) -> ResultType<(secretbox::Key, [u8; box_::PUBLICKEYBYTES])> {
        let Some(message::Union::PublicKey(pk)) = &reply.union else {
            bail!("Secure host requires PublicKey as handshake reply");
        };
        if reply
            .special_fields
            .unknown_fields()
            .iter()
            .next()
            .is_some()
            || pk.special_fields.unknown_fields().iter().next().is_some()
            || pk.kx_version != 1
            || pk.asymmetric_value.len() != box_::PUBLICKEYBYTES
            || pk.symmetric_value.len() != secretbox::KEYBYTES + box_::MACBYTES
        {
            bail!("Secure host requires a complete v1 key exchange");
        }
        let mut initiator = [0; box_::PUBLICKEYBYTES];
        initiator.copy_from_slice(&pk.asymmetric_value);
        let key = Encrypt::decode(&pk.symmetric_value, &pk.asymmetric_value, &self.secret)?;
        Ok((key, initiator))
    }
}

pub fn public_profile(id: &str, pk: &[u8], address: SocketAddr) -> ResultType<String> {
    if pk.len() != sign::PUBLICKEYBYTES
        || !(6..=20).contains(&id.len())
        || !id.bytes().all(|c| c.is_ascii_digit())
    {
        bail!("Invalid public identity");
    }
    Ok(serde_json::json!({
        "id": format!("official-{id}"), "name": "Windows secure entry", "mode":"direct",
        "target":address.to_string(), "server":"", "serverKey":"",
        "peerId":id, "peerPublicKey":STANDARD.encode(pk),
        "peerFingerprint":format!("{:x}", Sha256::digest(pk))
    })
    .to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Login,
    Heartbeat,
    Input,
    InputRequest,
    VideoSettings,
    Close,
    Denied,
}

pub fn classify_message(message: &Message) -> Request {
    if message
        .special_fields
        .unknown_fields()
        .iter()
        .next()
        .is_some()
    {
        return Request::Denied;
    }
    match &message.union {
        Some(message::Union::LoginRequest(_)) => Request::Login,
        Some(message::Union::TestDelay(_)) => Request::Heartbeat,
        Some(message::Union::OrdInputEvent(_)) => Request::Input,
        Some(message::Union::OrdVideoSettings(settings))
            if video_settings::Settings::parse(settings, 0).is_some() =>
        {
            Request::VideoSettings
        }
        Some(message::Union::OrdInputRequest(request))
            if request.version == 1
                && request.scope == "windows_primary"
                && request.request_id > 0
                && request
                    .special_fields
                    .unknown_fields()
                    .iter()
                    .next()
                    .is_none() =>
        {
            Request::InputRequest
        }
        Some(message::Union::Misc(m))
            if m.special_fields.unknown_fields().iter().next().is_none()
                && matches!(&m.union, Some(misc::Union::CloseReason(_))) =>
        {
            Request::Close
        }
        _ => Request::Denied,
    }
}

pub fn valid_login(lr: &LoginRequest, id: &str) -> bool {
    lr.username == id && lr.union.is_none() && lr.password.is_empty() && lr.os_login.is_none()
        && lr.hwid.is_empty() && lr.avatar.is_empty()
        && !lr.my_id.is_empty() && lr.my_id.len() <= 64 && lr.my_name.len() <= 128
        && (lr.ord_input_version == 0 || (lr.ord_input_version == 2 && requests_video(lr)))
        && (lr.ord_video_settings.is_none() || initial_video_settings(lr).is_some())
        && lr.special_fields.unknown_fields().iter().next().is_none()
        && lr.my_id.chars().chain(lr.my_name.chars()).all(|c|
            !c.is_control() && !matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
}

pub fn requests_video(lr: &LoginRequest) -> bool {
    lr.option
        .as_ref()
        .and_then(|option| option.supported_decoding.as_ref())
        .is_some_and(|decoding| {
            decoding.ability_vp8 == 1 && decoding.prefer.enum_value() == Ok(PreferCodec::VP8)
        })
}

pub fn requests_input(lr: &LoginRequest) -> bool {
    lr.ord_input_version == 2 && requests_video(lr)
}

pub fn initial_video_settings(lr: &LoginRequest) -> Option<video_settings::Settings> {
    if !requests_input(lr) || lr.ord_access.is_some() {
        return None;
    }
    video_settings::Settings::parse(lr.ord_video_settings.as_ref()?, 0)
        .filter(|settings| settings.request_id == 1)
}

pub fn updated_video_settings(
    lr: &LoginRequest,
    authorized: bool,
    previous_id: u64,
    request: &base::message_proto::OrdVideoSettings,
) -> Option<video_settings::Settings> {
    if !authorized || previous_id == 0 || initial_video_settings(lr).is_none() {
        return None;
    }
    video_settings::Settings::parse(request, previous_id)
}

pub fn approved_configurable_video_peer_info(version: &str, width: i32, height: i32) -> Message {
    let mut message = approved_control_peer_info(version, width, height);
    if let Some(message::Union::LoginResponse(login)) = message.union.as_mut() {
        if let Some(base::message_proto::login_response::Union::PeerInfo(peer)) =
            login.union.as_mut()
        {
            peer.platform_additions = serde_json::json!({
                "ord_secure_host": 1, "media": true, "video_codec": "vp8",
                "input_scope": "windows_primary", "input_version": 2, "video_settings_version": 1
            })
            .to_string();
        }
    }
    message
}

pub fn permission_snapshot() -> Vec<Message> {
    [
        Permission::Keyboard,
        Permission::Clipboard,
        Permission::Audio,
        Permission::File,
        Permission::Restart,
        Permission::Recording,
        Permission::BlockInput,
        Permission::PrivacyMode,
    ]
    .into_iter()
    .map(|permission| {
        let mut misc = Misc::new();
        misc.set_permission_info(PermissionInfo {
            permission: permission.into(),
            enabled: false,
            ..Default::default()
        });
        let mut message = Message::new();
        message.set_misc(misc);
        message
    })
    .collect()
}

pub fn approved_peer_info(version: &str) -> Message {
    let mut login = LoginResponse::new();
    login.set_peer_info(PeerInfo {
        platform: "Windows".to_owned(),
        version: version.to_owned(),
        platform_additions:
            serde_json::json!({"ord_secure_host":1,"media":false,"input_scope":"none"}).to_string(),
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_login_response(login);
    message
}

pub fn approved_video_peer_info(version: &str, width: i32, height: i32) -> Message {
    let mut login = LoginResponse::new();
    login.set_peer_info(PeerInfo {
        platform: "Windows".to_owned(),
        version: version.to_owned(),
        displays: vec![DisplayInfo {
            width,
            height,
            online: true,
            ..Default::default()
        }],
        current_display: 0,
        platform_additions: serde_json::json!({
            "ord_secure_host": 1, "media": true, "video_codec": "vp8", "input_scope": "none"
        })
        .to_string(),
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_login_response(login);
    message
}

pub fn approved_control_peer_info(version: &str, width: i32, height: i32) -> Message {
    let mut login = LoginResponse::new();
    login.set_peer_info(PeerInfo {
        platform: "Windows".to_owned(),
        version: version.to_owned(),
        displays: vec![DisplayInfo {
            width,
            height,
            online: true,
            ..Default::default()
        }],
        current_display: 0,
        platform_additions: serde_json::json!({
            "ord_secure_host": 1, "media": true, "video_codec": "vp8",
            "input_scope": "windows_primary", "input_version": 2
        })
        .to_string(),
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_login_response(login);
    message
}
