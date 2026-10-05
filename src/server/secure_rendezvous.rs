use super::secure_rendezvous_policy::decode_source;
pub use super::secure_rendezvous_policy::RendezvousConfig;
use hbb_common::{bail, ResultType};
pub use runtime::start;
use std::net::SocketAddr;
mod runtime {
    use super::*;
    use crate::server::{secure_host, ConnectionMeta, ServerPtr};
    use hbb_common::{
        anyhow::Context,
        config::{Config, CONNECT_TIMEOUT},
        log,
        protobuf::Message as _,
        rendezvous_proto::*,
        socket_client, tcp, timeout, tokio, AddrMangle, Stream,
    };

    async fn signal(config: &RendezvousConfig) -> ResultType<Stream> {
        if secure_host::is_stopped() {
            bail!("Secure host stopped");
        }
        let mut stream = socket_client::connect_tcp(&*config.server, CONNECT_TIMEOUT).await?;
        stream.set_max_packet_length(65536);
        crate::secure_tcp_required(&mut stream, &config.key).await?;
        if !matches!(&stream, Stream::Tcp(_)) || !stream.is_secured() {
            bail!("Secure rendezvous requires an authenticated encrypted TCP stream");
        }
        Ok(stream)
    }
    async fn accept(server: ServerPtr, stream: Stream, source: SocketAddr) -> ResultType<()> {
        if secure_host::is_stopped() {
            bail!("Secure host stopped");
        }
        crate::server::create_tcp_connection(
            server,
            stream,
            source,
            true,
            ConnectionMeta::default(),
        )
        .await
    }
    async fn relay(
        config: &RendezvousConfig,
        server: ServerPtr,
        source: SocketAddr,
        encoded: Vec<u8>,
        uuid: String,
        initiate: bool,
    ) -> ResultType<()> {
        if uuid.is_empty() || uuid.len() > 128 {
            bail!("Invalid relay session ID");
        }
        let mut signaling = signal(config).await?;
        let mut reply = RelayResponse {
            socket_addr: encoded.into(),
            version: crate::VERSION.into(),
            ..Default::default()
        };
        if initiate {
            reply.uuid = uuid.clone();
            reply.relay_server = config.relay.clone();
            reply.set_id(Config::get_id());
        }
        let mut msg = RendezvousMessage::new();
        msg.set_relay_response(reply);
        timeout(CONNECT_TIMEOUT, signaling.send(&msg)).await??;
        let mut stream = socket_client::connect_tcp(&*config.relay, CONNECT_TIMEOUT).await?;
        let mut msg = RendezvousMessage::new();
        msg.set_request_relay(RequestRelay {
            uuid,
            licence_key: config.key.clone(),
            ..Default::default()
        });
        timeout(CONNECT_TIMEOUT, stream.send(&msg)).await??;
        accept(server, stream, source).await
    }
    async fn direct(
        config: &RendezvousConfig,
        server: ServerPtr,
        source: SocketAddr,
        encoded: Vec<u8>,
        local: bool,
    ) -> ResultType<()> {
        let mut signaling = signal(config).await?;
        let address = signaling.local_addr();
        if address.ip().is_unspecified() {
            bail!("Secure coordination requires concrete local address");
        }
        // Reuse only the authenticated signaling socket's concrete interface and ephemeral port.
        let outbound = match socket_client::connect_tcp_local(source, Some(address), 30).await {
            Ok(stream) => Some(stream),
            Err(error) => {
                log::trace!("Coordinated outbound probe did not connect: {error}");
                None
            }
        };
        let mut msg = RendezvousMessage::new();
        if local {
            msg.set_local_addr(LocalAddr {
                socket_addr: encoded.into(),
                local_addr: AddrMangle::encode(address).into(),
                relay_server: config.relay.clone(),
                id: Config::get_id(),
                version: crate::VERSION.into(),
                ..Default::default()
            });
        } else {
            msg.set_punch_hole_sent(PunchHoleSent {
                socket_addr: encoded.into(),
                id: Config::get_id(),
                relay_server: config.relay.clone(),
                version: crate::VERSION.into(),
                ..Default::default()
            });
        }
        timeout(CONNECT_TIMEOUT, signaling.send(&msg)).await??;
        drop(signaling);
        if let Some(stream) = outbound {
            return accept(server, stream, source).await;
        }
        let listener = tcp::new_listener(address, true).await?;
        let (socket, actual) = timeout(CONNECT_TIMEOUT, listener.accept()).await??;
        if actual.ip() != source.ip() {
            bail!("Coordinated TCP source mismatch");
        }
        socket.set_nodelay(true)?;
        accept(server, Stream::from(socket, address), actual).await
    }
    async fn request(
        config: RendezvousConfig,
        server: ServerPtr,
        message: rendezvous_message::Union,
    ) -> ResultType<()> {
        match message {
            rendezvous_message::Union::RequestRelay(rr) => {
                if !rr.secure {
                    bail!("Unsecured relay request refused");
                }
                let source = decode_source(&rr.socket_addr)?;
                config.validate_request(source, &rr.relay_server)?;
                relay(
                    &config,
                    server,
                    source,
                    rr.socket_addr.to_vec(),
                    rr.uuid,
                    false,
                )
                .await
            }
            rendezvous_message::Union::PunchHole(ph) => {
                let source = decode_source(&ph.socket_addr)?;
                config.validate_request(source, &ph.relay_server)?;
                if ph.udp_port != 0 || !ph.webrtc_sdp_offer.is_empty() {
                    bail!("Only coordinated TCP is supported");
                }
                if ph.force_relay || ph.nat_type.enum_value() == Ok(NatType::SYMMETRIC) {
                    relay(
                        &config,
                        server,
                        source,
                        ph.socket_addr.to_vec(),
                        uuid::Uuid::new_v4().to_string(),
                        true,
                    )
                    .await
                } else {
                    direct(&config, server, source, ph.socket_addr.to_vec(), false).await
                }
            }
            rendezvous_message::Union::FetchLocalAddr(fla) => {
                let source = decode_source(&fla.socket_addr)?;
                config.validate_request(source, &fla.relay_server)?;
                direct(&config, server, source, fla.socket_addr.to_vec(), true).await
            }
            _ => bail!("Unexpected secure rendezvous request"),
        }
    }
    pub async fn start(config: RendezvousConfig, server: ServerPtr) -> ResultType<()> {
        let mut stream = signal(&config).await?;
        let mut registration = RendezvousMessage::new();
        registration.set_register_pk(RegisterPk {
            id: Config::get_id(),
            uuid: hbb_common::get_uuid().into(),
            pk: Config::get_key_pair().1.into(),
            ..Default::default()
        });
        timeout(CONNECT_TIMEOUT, stream.send(&registration)).await??;
        let bytes = timeout(CONNECT_TIMEOUT, stream.next())
            .await?
            .context("Registration closed")??;
        crate::server::secure_rendezvous_policy::validate_registration(&bytes)?;
        if let Ok(path) = std::env::var("ORD_SECURE_ID_PROFILE_OUT") {
            if !path.is_empty() {
                let pk = Config::get_key_pair().1;
                let base = crate::server::secure_host_policy::public_profile(
                    &Config::get_id(),
                    &pk,
                    "127.0.0.1:1".parse()?,
                )?;
                let mut profile: serde_json::Value = serde_json::from_str(&base)?;
                profile["mode"] = "id".into();
                profile["target"] = Config::get_id().into();
                profile["server"] = config.server.clone().into();
                profile["serverKey"] = config.key.clone().into();
                profile["relayServer"] = config.relay.clone().into();
                std::fs::write(path, serde_json::to_string(&profile)?)?;
            }
        }
        log::info!("Secure host ID registered with configured rendezvous");
        let limit = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
        let mut last_received = std::time::Instant::now();
        loop {
            if secure_host::is_stopped() {
                return Ok(());
            }
            let bytes = match timeout(1000, stream.next()).await {
                Ok(value) => value.context("Secure rendezvous closed")??,
                Err(_) => {
                    if last_received.elapsed().as_secs() > 90 {
                        bail!("Secure rendezvous heartbeat timed out");
                    }
                    continue;
                }
            };
            last_received = std::time::Instant::now();
            if bytes.is_empty() {
                timeout(CONNECT_TIMEOUT, stream.send_bytes(bytes.into())).await??;
                continue;
            }
            if bytes.len() > 65536 {
                bail!("Oversized rendezvous message");
            }
            let message = RendezvousMessage::parse_from_bytes(&bytes)?;
            let Some(message) = message.union else {
                bail!("Empty rendezvous message");
            };
            let Ok(permit) = limit.clone().try_acquire_owned() else {
                log::trace!("Secure rendezvous active-session limit reached; request declined");
                continue;
            };
            let config = config.clone();
            let server = server.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(err) = request(config, server, message).await {
                    log::trace!("Secure rendezvous request refused: {err}");
                }
            });
        }
    }
}
