//! Loopback-only renderer fixture, not an official host or an approval implementation.
//! Sends a supplied 64x48 VP8 test keyframe; never captures the desktop.
use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use bytes::BytesMut;
use protobuf::Message as _;
use remote_controller_core::{
    protos::{message::*, rendezvous::IdPk},
    upstream_crypto::{Encrypt, KxTranscript},
};
use sodiumoxide::crypto::{box_, sign};
use std::{
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

fn send(stream: &mut TcpStream, bytes: &[u8]) -> Result<()> {
    let count = if bytes.len() <= 63 {
        1
    } else if bytes.len() <= 16383 {
        2
    } else if bytes.len() <= 4194303 {
        3
    } else {
        4
    };
    let header = ((bytes.len() as u32) << 2 | (count - 1)).to_le_bytes();
    stream.write_all(&header[..count as usize])?;
    stream.write_all(bytes)?;
    Ok(())
}

fn receive(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut header = [0; 4];
    stream.read_exact(&mut header[..1])?;
    let count = (header[0] & 3) as usize + 1;
    stream.read_exact(&mut header[1..count])?;
    let len = (u32::from_le_bytes(header) >> 2) as usize;
    if len > 65536 {
        bail!("oversized fixture request");
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn encrypted(stream: &mut TcpStream, cipher: &mut Encrypt, message: Message) -> Result<()> {
    send(stream, &cipher.enc(&message.write_to_bytes()?))
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        bail!("usage: screen_fixture <synthetic-64x48.vp8> <public-profile.json>");
    }
    let frame = fs::read(&args[1])?;
    if frame.len() < 10
        || frame.len() > 2 * 1024 * 1024
        || frame[0] & 1 != 0
        || frame[3..6] != [0x9d, 1, 0x2a]
        || (u16::from_le_bytes([frame[6], frame[7]]) & 0x3fff) != 64
        || (u16::from_le_bytes([frame[8], frame[9]]) & 0x3fff) != 48
    {
        bail!("only a synthetic 64x48 VP8 keyframe is accepted");
    }
    sodiumoxide::init().map_err(|_| anyhow::anyhow!("crypto init failed"))?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let (pk, sk) = sign::gen_keypair();
    fs::write(&args[2], serde_json::json!({
        "id":"vp8-render-fixture", "name":"VP8 test frame", "mode":"direct",
        "target":listener.local_addr()?.to_string(), "server":"", "serverKey":"", "peerFingerprint":"",
        "peerId":"123456788", "peerPublicKey":STANDARD.encode(pk.0)
    }).to_string())?;
    println!("Synthetic fixture ready on {}", listener.local_addr()?);
    let deadline = Instant::now() + Duration::from_secs(900);
    let mut stream = loop {
        if Instant::now() >= deadline {
            bail!("fixture accept timed out");
        }
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25))
            }
            Err(e) => return Err(e.into()),
        }
    };
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let (ephemeral, secret) = box_::gen_keypair();
    let mut message = Message::new();
    message.set_signed_id(SignedId {
        id: sign::sign(
            &IdPk {
                id: "123456788".into(),
                pk: ephemeral.0.to_vec(),
                kx_version: 1,
                ..Default::default()
            }
            .write_to_bytes()?,
            &sk,
        ),
        ..Default::default()
    });
    send(&mut stream, &message.write_to_bytes()?)?;
    let reply = Message::parse_from_bytes(&receive(&mut stream)?)?;
    let Some(message::Union::PublicKey(key)) = reply.union else {
        bail!("expected key");
    };
    if key.kx_version != 1 {
        bail!("v1 required");
    }
    let decoded = Encrypt::decode(&key.symmetric_value, &key.asymmetric_value, &secret)?;
    let mut cipher = Encrypt::new_split(
        decoded,
        false,
        &KxTranscript {
            initiator_pk: &key.asymmetric_value,
            responder_pk: &ephemeral.0,
            advertised: 1,
            picked: 1,
        },
    )?;
    let mut message = Message::new();
    message.set_hash(Hash {
        salt: "fixture".into(),
        challenge: "synthetic-screen-fixture".into(),
        ..Default::default()
    });
    encrypted(&mut stream, &mut cipher, message)?;
    let mut bytes = BytesMut::from(receive(&mut stream)?.as_slice());
    if bytes.len() < 16 {
        bail!("encrypted login required");
    }
    cipher.dec(&mut bytes)?;
    let login = Message::parse_from_bytes(&bytes)?;
    if !login.has_login_request()
        || !login.login_request().password.is_empty()
        || login.login_request().option.supported_decoding.ability_vp8 != 1
    {
        bail!("video fixture requires empty-password VP8 login");
    }
    let mut response = LoginResponse::new();
    response.set_peer_info(PeerInfo {
        platform: "Test fixture".into(),
        displays: vec![DisplayInfo {
            width: 64,
            height: 48,
            online: true,
            ..Default::default()
        }],
        platform_additions:
            r#"{"ord_secure_host":1,"media":true,"video_codec":"vp8","input_scope":"none"}"#.into(),
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_login_response(response);
    encrypted(&mut stream, &mut cipher, message)?;
    for number in 0..240 {
        let mut video = VideoFrame::new();
        video.set_vp8s(EncodedVideoFrames {
            frames: vec![EncodedVideoFrame {
                data: frame.clone(),
                key: true,
                pts: number * 250,
                ..Default::default()
            }],
            ..Default::default()
        });
        let mut message = Message::new();
        message.set_video_frame(video);
        if encrypted(&mut stream, &mut cipher, message).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    println!("Synthetic fixture ended");
    Ok(())
}
