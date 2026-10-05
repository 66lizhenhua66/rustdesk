use crate::{
    protos::rendezvous::{
        relay_response, rendezvous_message::Union, KeyExchange, KxParams, PunchHoleRequest,
        RendezvousMessage, RequestRelay,
    },
    session::{io_failure, status, ControllerSession, Failure, Wire},
    upstream_crypto::{self, Encrypt, KxTranscript, KX_PARAMS_DOMAIN},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use protobuf::Message as _;
use socket2::{Domain, Protocol, Socket, Type};
use sodiumoxide::{
    crypto::{secretbox, sign},
    randombytes,
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, ToSocketAddrs},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub(crate) struct Config {
    force_relay: bool,
    server: String,
    server_key: sign::PublicKey,
    relay: String,
    key_text: String,
}
impl Config {
    pub(crate) fn parse(
        force_relay: bool,
        server: Option<String>,
        key: Option<String>,
        relay: Option<String>,
    ) -> Option<Self> {
        let (server, key_text, relay) = (server?, key?, relay?);
        if !valid_address(&server) || !valid_address(&relay) {
            return None;
        }
        let server_key = sign::PublicKey::from_slice(&STANDARD.decode(&key_text).ok()?)?;
        Some(Self {
            force_relay,
            server,
            server_key,
            relay,
            key_text,
        })
    }
}
fn valid_address(value: &str) -> bool {
    if let Ok(a) = value.parse::<SocketAddr>() {
        return a.port() != 0 && !a.ip().is_unspecified() && !a.ip().is_multicast();
    }
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        && port.parse::<u16>().is_ok_and(|p| p != 0)
}
static RESOLVERS: AtomicUsize = AtomicUsize::new(0);
fn resolve(
    task: &ControllerSession,
    deadline: Instant,
    value: &str,
) -> Result<Vec<SocketAddr>, Failure> {
    if let Ok(a) = value.parse() {
        return Ok(vec![a]);
    }
    RESOLVERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < 4).then_some(n + 1)
        })
        .map_err(|_| Failure::failed("RESOLVER_BUSY", "Too many pending name lookups"))?;
    let (tx, rx) = mpsc::sync_channel(1);
    let value = value.to_owned();
    if thread::Builder::new()
        .name("controller-dns".into())
        .spawn(move || {
            let result = value
                .to_socket_addrs()
                .map(|v| v.take(8).collect::<Vec<_>>());
            let _ = tx.send(result);
            RESOLVERS.fetch_sub(1, Ordering::AcqRel);
        })
        .is_err()
    {
        RESOLVERS.fetch_sub(1, Ordering::AcqRel);
        return Err(Failure::failed(
            "RESOLVE_FAILED",
            "Cannot start name lookup",
        ));
    }
    loop {
        match rx.recv_timeout(status(task, deadline)?) {
            Ok(Ok(v)) if !v.is_empty() => return Ok(v),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            _ => {
                return Err(Failure::failed(
                    "RESOLVE_FAILED",
                    "Server name lookup failed",
                ))
            }
        }
    }
}
fn tcp(
    task: &ControllerSession,
    deadline: Instant,
    addr: SocketAddr,
    bind: Option<SocketAddr>,
) -> Result<TcpStream, Failure> {
    status(task, deadline)?;
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))
        .map_err(|_| io_failure(task, deadline))?;
    socket
        .set_reuse_address(true)
        .map_err(|_| io_failure(task, deadline))?;
    if let Some(local) = bind {
        socket
            .bind(&local.into())
            .map_err(|_| io_failure(task, deadline))?;
    }
    let cloned: TcpStream = socket
        .try_clone()
        .map_err(|_| io_failure(task, deadline))?
        .into();
    *task.socket.lock().unwrap() = Some(cloned);
    status(task, deadline)?;
    socket
        .connect_timeout(
            &addr.into(),
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(700)),
        )
        .map_err(|_| io_failure(task, deadline))?;
    status(task, deadline)?;
    Ok(socket.into())
}
fn named_tcp(
    task: &ControllerSession,
    deadline: Instant,
    addr: &str,
) -> Result<TcpStream, Failure> {
    for addr in resolve(task, deadline, addr)? {
        if let Ok(s) = tcp(task, deadline, addr, None) {
            return Ok(s);
        }
        status(task, deadline)?;
    }
    Err(io_failure(task, deadline))
}
fn protocol() -> Failure {
    Failure::failed(
        "RENDEZVOUS_PROTOCOL_FAILED",
        "Invalid coordination response",
    )
}
fn identity() -> Failure {
    Failure::failed(
        "SERVER_IDENTITY_INVALID",
        "Coordination server signature is invalid",
    )
}
fn secure(
    task: &ControllerSession,
    deadline: Instant,
    config: &Config,
) -> Result<(Wire, Encrypt), Failure> {
    let mut wire = Wire::new(named_tcp(task, deadline, &config.server)?);
    let message = RendezvousMessage::parse_from_bytes(&wire.receive(task, deadline)?)
        .map_err(|_| identity())?;
    let Some(Union::KeyExchange(ex)) = message.union else {
        return Err(identity());
    };
    if ex.keys.len() != 1 || ex.version > 1 {
        return Err(identity());
    }
    let signed = sign::verify(&ex.keys[0], &config.server_key).map_err(|_| identity())?;
    let pk: [u8; 32] = signed.try_into().map_err(|_| identity())?;
    if pk[31] & 0x80 != 0 || !ex.signed_params.is_empty() || ex.version != 0 {
        let signed = sign::verify(&ex.signed_params, &config.server_key).map_err(|_| identity())?;
        let params =
            KxParams::parse_from_bytes(signed.strip_prefix(KX_PARAMS_DOMAIN).ok_or_else(identity)?)
                .map_err(|_| identity())?;
        if params.pk != pk || params.version != ex.version {
            return Err(identity());
        }
    }
    let (our_pk, sealed, key) = upstream_crypto::create_symmetric_key_msg(pk);
    let mut response = RendezvousMessage::new();
    response.set_key_exchange(KeyExchange {
        keys: vec![our_pk.to_vec(), sealed.to_vec()],
        version: ex.version,
        ..Default::default()
    });
    wire.send(
        task,
        deadline,
        &response.write_to_bytes().map_err(|_| protocol())?,
    )?;
    let cipher = if ex.version == 0 {
        Encrypt::new(key)
    } else {
        Encrypt::new_split(
            key,
            true,
            &KxTranscript {
                initiator_pk: &our_pk,
                responder_pk: &pk,
                advertised: ex.version,
                picked: 1,
            },
        )
        .map_err(|_| identity())?
    };
    Ok((wire, cipher))
}
fn send(
    task: &ControllerSession,
    deadline: Instant,
    wire: &mut Wire,
    cipher: &mut Encrypt,
    message: RendezvousMessage,
) -> Result<(), Failure> {
    wire.send(
        task,
        deadline,
        &cipher.enc(&message.write_to_bytes().map_err(|_| protocol())?),
    )
}
fn receive(
    task: &ControllerSession,
    deadline: Instant,
    wire: &mut Wire,
    cipher: &mut Encrypt,
) -> Result<RendezvousMessage, Failure> {
    let mut bytes = wire.receive(task, deadline)?;
    if bytes.len() < secretbox::MACBYTES || cipher.2 == u64::MAX {
        return Err(protocol());
    }
    cipher.dec(&mut bytes).map_err(|_| protocol())?;
    RendezvousMessage::parse_from_bytes(&bytes).map_err(|_| protocol())
}
fn verify_peer(task: &ControllerSession, config: &Config, signed: &[u8]) -> Result<(), Failure> {
    let (id, key, _, _) =
        upstream_crypto::decode_id_pk_dtls(signed, &config.server_key).map_err(|_| {
            Failure::failed(
                "PEER_IDENTITY_INVALID",
                "Invalid server-signed peer identity",
            )
        })?;
    if id != task.peer_id || key != task.peer_key.0 {
        return Err(Failure::failed(
            "PEER_IDENTITY_MISMATCH",
            "Server-signed peer identity differs from pinned identity",
        ));
    }
    Ok(())
}
fn address(bytes: &[u8]) -> Result<SocketAddr, Failure> {
    let addr = if bytes.len() == 18 {
        let octets: [u8; 16] = bytes[..16].try_into().map_err(|_| protocol())?;
        SocketAddr::new(
            IpAddr::V6(Ipv6Addr::from(octets)),
            u16::from_le_bytes([bytes[16], bytes[17]]),
        )
    } else if !bytes.is_empty() && bytes.len() <= 16 {
        let mut padded = [0; 16];
        padded[..bytes.len()].copy_from_slice(bytes);
        let n = u128::from_le_bytes(padded);
        let tm = (n >> 17) & u32::MAX as u128;
        let ip = (n >> 49)
            .checked_sub(tm)
            .filter(|ip| *ip <= u32::MAX as u128)
            .ok_or_else(protocol)? as u32;
        let port = (n & 0x1FFFF)
            .checked_sub(tm & 0xFFFF)
            .filter(|p| *p <= u16::MAX as u128)
            .ok_or_else(protocol)? as u16;
        SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip.to_le_bytes())), port)
    } else {
        return Err(protocol());
    };
    if addr.port() == 0 || addr.ip().is_unspecified() || addr.ip().is_multicast() {
        return Err(protocol());
    }
    Ok(addr)
}
fn relay(
    task: &ControllerSession,
    deadline: Instant,
    config: &Config,
    uuid: String,
) -> Result<(Wire, &'static str), Failure> {
    if uuid.is_empty() || uuid.len() > 128 {
        return Err(protocol());
    }
    let mut wire = Wire::new(named_tcp(task, deadline, &config.relay)?);
    let mut request = RendezvousMessage::new();
    request.set_request_relay(RequestRelay {
        id: task.peer_id.clone(),
        uuid,
        licence_key: config.key_text.clone(),
        ..Default::default()
    });
    wire.send(
        task,
        deadline,
        &request.write_to_bytes().map_err(|_| protocol())?,
    )?;
    Ok((wire, "relay"))
}
fn request_relay(
    task: &ControllerSession,
    deadline: Instant,
    config: &Config,
) -> Result<(Wire, &'static str), Failure> {
    let (mut wire, mut cipher) = secure(task, deadline, config)?;
    let uuid = randombytes::randombytes(16)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut request = RendezvousMessage::new();
    request.set_request_relay(RequestRelay {
        id: task.peer_id.clone(),
        uuid: uuid.clone(),
        relay_server: config.relay.clone(),
        secure: true,
        ..Default::default()
    });
    send(task, deadline, &mut wire, &mut cipher, request)?;
    let Some(Union::RelayResponse(response)) =
        receive(task, deadline, &mut wire, &mut cipher)?.union
    else {
        return Err(protocol());
    };
    if !response.refuse_reason.is_empty() {
        return Err(Failure::failed("RELAY_REJECTED", "Relay request rejected"));
    }
    if (!response.uuid.is_empty() && response.uuid != uuid)
        || (!response.relay_server.is_empty() && response.relay_server != config.relay)
    {
        return Err(protocol());
    }
    match response.union {
        Some(relay_response::Union::Pk(pk)) => verify_peer(task, config, &pk)?,
        Some(relay_response::Union::Id(id)) if id != task.peer_id => return Err(protocol()),
        _ => {}
    }
    relay(task, deadline, config, uuid)
}
pub(crate) fn connect(
    task: &ControllerSession,
    deadline: Instant,
    config: &Config,
) -> Result<(Wire, &'static str), Failure> {
    let (mut wire, mut cipher) = secure(task, deadline, config)?;
    let local = wire
        .stream
        .local_addr()
        .map_err(|_| io_failure(task, deadline))?;
    let mut request = RendezvousMessage::new();
    request.set_punch_hole_request(PunchHoleRequest {
        id: task.peer_id.clone(),
        force_relay: config.force_relay,
        licence_key: config.key_text.clone(),
        version: "1.4.6".into(),
        ..Default::default()
    });
    send(task, deadline, &mut wire, &mut cipher, request)?;
    match receive(task, deadline, &mut wire, &mut cipher)?.union {
        Some(Union::RelayResponse(response)) => {
            if !response.refuse_reason.is_empty() {
                return Err(Failure::failed("RELAY_REJECTED", "Relay request rejected"));
            }
            if response.relay_server != config.relay {
                return Err(Failure::failed(
                    "RELAY_SERVER_MISMATCH",
                    "Relay differs from configured server",
                ));
            }
            let Some(relay_response::Union::Pk(pk)) = response.union else {
                return Err(Failure::failed(
                    "PEER_IDENTITY_INVALID",
                    "Missing server-signed peer identity",
                ));
            };
            verify_peer(task, config, &pk)?;
            relay(task, deadline, config, response.uuid)
        }
        Some(Union::PunchHoleResponse(response)) => {
            if !response.other_failure.is_empty() || response.socket_addr.is_empty() {
                return Err(Failure::failed("PEER_UNAVAILABLE", "Peer is unavailable"));
            }
            verify_peer(task, config, &response.pk)?;
            if !response.relay_server.is_empty() && response.relay_server != config.relay {
                return Err(Failure::failed(
                    "RELAY_SERVER_MISMATCH",
                    "Relay differs from configured server",
                ));
            }
            if !config.force_relay {
                let addr = address(&response.socket_addr)?;
                let bind =
                    (!response.is_local() && addr.is_ipv4() == local.is_ipv4()).then_some(local);
                if let Ok(stream) = tcp(task, deadline, addr, bind) {
                    return Ok((Wire::new(stream), "id_direct"));
                }
                status(task, deadline)?;
            }
            drop(wire);
            request_relay(task, deadline, config)
        }
        _ => Err(protocol()),
    }
}
#[cfg(test)]
mod tests {
    use super::address;
    #[test]
    fn upstream_timestamped_address_decodes_without_timestamp_port_bits() {
        let timestamp = 0x12345678u128;
        let ip = u32::from_le_bytes([192, 168, 1, 42]) as u128;
        let encoded = ((ip + timestamp) << 49) | (timestamp << 17) | (21118 + (timestamp & 0xffff));
        let decoded = address(&encoded.to_le_bytes())
            .ok()
            .expect("upstream address must decode");
        assert_eq!(decoded.to_string(), "192.168.1.42:21118");
    }
}
