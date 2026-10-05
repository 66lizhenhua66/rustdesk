use hbb_common::{
    bail,
    base64::{engine::general_purpose::STANDARD, Engine as _},
    ResultType,
};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone)]
pub struct RendezvousConfig {
    pub server: String,
    pub key: String,
    pub relay: String,
    allowed: Vec<IpAddr>,
}
impl RendezvousConfig {
    pub fn parse(server: &str, key: &str, relay: &str, sources: &str) -> ResultType<Option<Self>> {
        if server.is_empty() && key.is_empty() && relay.is_empty() {
            return Ok(None);
        }
        for address in [server, relay] {
            let Some((host, port)) = address.rsplit_once(':') else {
                bail!("Secure rendezvous requires explicit host:port endpoints");
            };
            if host.is_empty()
                || host.contains(['/', '@', ' ', '\\'])
                || port.parse::<u16>().unwrap_or(0) == 0
            {
                bail!("Invalid secure rendezvous endpoint");
            }
        }
        if STANDARD.decode(key)?.len() != 32 {
            bail!("Secure rendezvous requires a pinned 32-byte server key");
        }
        let mut allowed = Vec::new();
        for source in sources.split(',') {
            let ip: IpAddr = source.trim().parse()?;
            if ip.is_unspecified() || ip.is_multicast() || allowed.len() >= 16 {
                bail!("Secure rendezvous requires at most 16 concrete source IPs");
            }
            allowed.push(ip);
        }
        Ok(Some(Self {
            server: server.into(),
            key: key.into(),
            relay: relay.into(),
            allowed,
        }))
    }
    pub fn validate_request(&self, source: SocketAddr, relay: &str) -> ResultType<()> {
        if source.port() == 0 || !self.allowed.contains(&source.ip()) {
            bail!("Secure rendezvous source is not allowed");
        }
        if !relay.is_empty() && relay != self.relay {
            bail!("Secure rendezvous relay differs from configured relay");
        }
        Ok(())
    }
}

// Decode with checked arithmetic: malformed network addresses must not panic.
pub fn decode_source(bytes: &[u8]) -> ResultType<SocketAddr> {
    if bytes.len() == 18 {
        let mut ip = [0; 16];
        ip.copy_from_slice(&bytes[..16]);
        return Ok(SocketAddr::new(
            std::net::Ipv6Addr::from(ip).into(),
            u16::from_le_bytes([bytes[16], bytes[17]]),
        ));
    }
    if bytes.is_empty() || bytes.len() > 16 {
        bail!("Invalid rendezvous source address");
    }
    let mut padded = [0; 16];
    padded[..bytes.len()].copy_from_slice(bytes);
    let number = u128::from_le_bytes(padded);
    let tm = (number >> 17) & u32::MAX as u128;
    let Some(ip) = (number >> 49).checked_sub(tm) else {
        bail!("Invalid rendezvous source IP");
    };
    let Some(port) = (number & 0x1ffff).checked_sub(tm & 0xffff) else {
        bail!("Invalid rendezvous source port");
    };
    if ip == 0 || ip > u32::MAX as u128 || port == 0 || port > u16::MAX as u128 {
        bail!("Invalid rendezvous source");
    }
    Ok(SocketAddr::new(
        std::net::Ipv4Addr::from((ip as u32).to_le_bytes()).into(),
        port as u16,
    ))
}

pub fn validate_registration(bytes: &[u8]) -> ResultType<()> {
    use hbb_common::protobuf::Message as _;
    use hbb_common::rendezvous_proto::{
        register_pk_response, rendezvous_message, RendezvousMessage,
    };
    if bytes.len() > 65536 {
        bail!("Oversized registration response");
    }
    let response = RendezvousMessage::parse_from_bytes(bytes)?;
    let Some(rendezvous_message::Union::RegisterPkResponse(response)) = response.union else {
        bail!("Expected registration response");
    };
    if response.result.enum_value() != Ok(register_pk_response::Result::OK) {
        bail!("Secure host ID registration refused: {:?}", response.result);
    }
    Ok(())
}
