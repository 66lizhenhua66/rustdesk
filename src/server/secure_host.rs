use super::{
    secure_host_policy::{self, IdentityHandshake},
    ConnectionMeta, Server, ServerPtr,
};
use base::message_proto::Message;
use hbb_common::{
    anyhow::Context,
    bail,
    config::{Config, CONNECT_TIMEOUT},
    log,
    protobuf::Message as _,
    tcp::{self, KxTranscript},
    timeout, tokio, ResultType, Stream,
};

fn new_server() -> ServerPtr {
    std::sync::Arc::new(std::sync::RwLock::new(Server {
        connections: Default::default(),
        services: Default::default(),
        id_count: hbb_common::rand::random::<i32>() % 1000 + 1000,
    }))
}

pub async fn start() -> ResultType<()> {
    let config = super::secure_rendezvous::RendezvousConfig::parse(
        &std::env::var("ORD_SECURE_RENDEZVOUS").unwrap_or_default(),
        &std::env::var("ORD_SECURE_SERVER_KEY").unwrap_or_default(),
        &std::env::var("ORD_SECURE_RELAY").unwrap_or_default(),
        &std::env::var("ORD_SECURE_ALLOW").unwrap_or_default(),
    )?;
    if !is_stopped() && (config.is_some() || !std::env::var("ORD_SECURE_LISTEN").unwrap_or_default().is_empty()) {
        match tokio::task::spawn_blocking(super::secure_display::recover_pending).await? {
            Ok(()) => {}
            Err(super::secure_display::DisplayError::ExternalChanged) => {
                log::warn!("Display recovery deferred: local display settings changed");
            }
            Err(error) => bail!("Pending display restoration failed: {error}"),
        }
    }
    if let Some(config) = config {
        let (sk, pk) = Config::get_key_pair();
        IdentityHandshake::new(&Config::get_id(), &sk, &pk)?;
        let server = new_server();
        tokio::select! {
            result = super::secure_rendezvous::start(config, server.clone()) => result,
            result = start_listener(Some(server.clone())) => result,
        }
    } else {
        start_listener(None).await
    }
}

async fn start_listener(server: Option<ServerPtr>) -> ResultType<()> {
    if is_stopped() {
        log::info!("Secure host stopped by local service setting");
        return std::future::pending().await;
    }
    let endpoint = std::env::var("ORD_SECURE_LISTEN").unwrap_or_default();
    let sources = std::env::var("ORD_SECURE_ALLOW").unwrap_or_default();
    let Some(config) = secure_host_policy::parse_listen(&endpoint, &sources)? else {
        log::info!("Secure host listener disabled");
        return std::future::pending().await;
    };
    let (sk, pk) = Config::get_key_pair();
    IdentityHandshake::new(&Config::get_id(), &sk, &pk)?;
    let listener = tcp::new_listener(config.address, false).await?;
    if let Ok(path) = std::env::var("ORD_SECURE_PROFILE_OUT") {
        if !path.is_empty() {
            let profile =
                secure_host_policy::public_profile(&Config::get_id(), &pk, config.address)?;
            std::fs::write(path, profile).context("Could not export secure host public profile")?;
        }
    }
    log::info!("Secure host listening on {}", config.address);
    let server = server.unwrap_or_else(new_server);
    loop {
        if is_stopped() {
            log::info!("Secure host listener stopped by local service setting");
            return Ok(());
        }
        let (socket, source) = match timeout(1000, listener.accept()).await {
            Ok(result) => result?,
            Err(_) => continue,
        };
        if is_stopped() {
            return Ok(());
        }
        if !config.permits(source.ip()) {
            continue;
        }
        let server = server.clone();
        tokio::spawn(async move {
            if let Err(err) = async {
                socket.set_nodelay(true)?;
                let local = socket.local_addr()?;
                super::create_tcp_connection(
                    server,
                    Stream::from(socket, local),
                    source,
                    true,
                    ConnectionMeta::default(),
                )
                .await
            }
            .await
            {
                log::trace!("Secure host connection from {} closed: {}", source, err);
            }
        });
    }
}

pub fn is_stopped() -> bool {
    hbb_common::config::option2bool("stop-service", &Config::get_option("stop-service"))
}

pub async fn identity_handshake(stream: &mut Stream) -> ResultType<()> {
    if is_stopped() {
        bail!("Secure host service stopped");
    }
    if !matches!(&*stream, Stream::Tcp(_)) {
        bail!("Secure host requires TCP");
    }
    let (sk, pk) = Config::get_key_pair();
    let handshake = IdentityHandshake::new(&Config::get_id(), &sk, &pk)?;
    timeout(CONNECT_TIMEOUT, stream.send(&handshake.message)).await??;
    let bytes = timeout(CONNECT_TIMEOUT, stream.next())
        .await?
        .context("Secure host key exchange ended early")??;
    if is_stopped() {
        bail!("Secure host service stopped");
    }
    let reply = Message::parse_from_bytes(&bytes)?;
    let (key, initiator) = handshake.finish(&reply)?;
    stream.set_negotiated_key(
        key,
        false,
        &KxTranscript {
            initiator_pk: &initiator,
            responder_pk: &handshake.public,
            advertised: 1,
            picked: 1,
        },
    )?;
    if !stream.is_secured() {
        bail!("Secure host key exchange did not secure the stream");
    }
    if is_stopped() {
        bail!("Secure host service stopped");
    }
    Ok(())
}
