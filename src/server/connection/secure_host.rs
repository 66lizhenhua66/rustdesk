use super::super::secure_host_policy::{self, Request};
use super::super::secure_video;
use super::*;

fn approve_pending(pending: &mut bool, authorized: &mut bool, requires_2fa: bool) -> bool {
    if !*pending || *authorized || requires_2fa {
        return false;
    }
    *pending = false;
    *authorized = true;
    true
}

impl Connection {
    async fn send_secure_video_error(&mut self) {
        let mut misc = Misc::new();
        misc.set_close_reason("Screen unavailable".to_owned());
        let mut message = Message::new();
        message.set_misc(misc);
        let _ = self.stream.send(&message).await;
    }

    async fn send_secure_permissions(&mut self) -> bool {
        for message in secure_host_policy::permission_snapshot() {
            if self.stream.send(&message).await.is_err() {
                return false;
            }
        }
        true
    }

    pub(super) async fn run_secure_host(
        &mut self,
        addr: SocketAddr,
        rx_from_cm: &mut mpsc::UnboundedReceiver<ipc::Data>,
        rx: &mut mpsc::UnboundedReceiver<(Instant, Arc<Message>)>,
        rx_video: &mut mpsc::UnboundedReceiver<(Instant, Arc<Message>)>,
        rx_from_authed: &mut mpsc::UnboundedReceiver<ipc::Data>,
    ) {
        self.stream.set_max_packet_length(64 * 1024);
        self.stream.set_send_timeout(2000);
        let addr = hbb_common::try_into_v4(addr);
        if super::super::secure_host::is_stopped()
            || !self.stream.is_secured()
            || !self.check_whitelist(&addr).await
        {
            self.on_close("Secure host admission failed", false).await;
            return;
        }
        self.ip = addr.ip().to_string();
        self.keyboard = false;
        self.clipboard = false;
        self.audio = false;
        self.file = false;
        self.restart = false;
        self.recording = false;
        self.block_input = false;
        self.privacy_mode = false;
        let mut hash = Message::new();
        hash.set_hash(self.hash.clone());
        if self.stream.send(&hash).await.is_err() {
            self.on_close("Secure host challenge failed", false).await;
            return;
        }
        if !self.send_secure_permissions().await {
            self.on_close("Secure host permission snapshot failed", false)
                .await;
            return;
        }

        let started = Instant::now();
        let login_deadline = started + LOGIN_GRACE;
        let session_deadline = started + Duration::from_secs(30 * 60);
        let mut stop_tick = time::interval(Duration::from_secs(1));
        let mut video_worker: Option<secure_video::Worker> = None;
        loop {
            if super::super::secure_host::is_stopped() {
                break;
            }
            tokio::select! {
                biased;
                _ = stop_tick.tick() => {
                    if super::super::secure_host::is_stopped() {
                        break;
                    }
                }
                _ = time::sleep_until(session_deadline) => break,
                _ = time::sleep_until(login_deadline), if !self.authorized => break,
                _ = unauthorized_evicted(&self.unauthorized_id) => break,
                Some(data) = rx_from_cm.recv() => {
                    match data {
                        ipc::Data::Authorize if approve_pending(
                            &mut self.secure_login_pending,
                            &mut self.authorized,
                            self.require_2fa.is_some(),
                        ) => {
                            self.unauthorized_id = None;
                            self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::Click);
                            let video_requested = secure_host_policy::requests_video(&self.lr);
                            if video_requested && !secure_video::allowed() {
                                self.send_secure_video_error().await;
                                break;
                            }
                            let video_dimensions = if video_requested {
                                let Ok(Ok(dimensions)) = tokio::task::spawn_blocking(secure_video::dimensions).await else {
                                    self.send_secure_video_error().await;
                                    break;
                                };
                                Some(dimensions)
                            } else {
                                None
                            };
                            let info = if let Some(dimensions) = video_dimensions {
                                secure_host_policy::approved_video_peer_info(
                                    VERSION, dimensions.width as _, dimensions.height as _)
                            } else {
                                secure_host_policy::approved_peer_info(VERSION)
                            };
                            if self.stream.send(&info).await.is_err() {
                                break;
                            }
                            if !self.send_secure_permissions().await {
                                break;
                            }
                            if let Some(dimensions) = video_dimensions {
                                match secure_video::start(dimensions) {
                                    Ok(worker) => video_worker = Some(worker),
                                    Err(_) => {
                                        self.send_secure_video_error().await;
                                        break;
                                    }
                                }
                            }
                        }
                        ipc::Data::Close => break,
                        ipc::Data::CmErr(_) => break,
                        ipc::Data::SwitchPermission { .. } => {
                            if !self.send_secure_permissions().await {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                Some(_) = rx.recv() => {}
                Some(_) = rx_video.recv() => {}
                Some(_) = rx_from_authed.recv() => {}
                frame = async {
                    match &mut video_worker {
                        Some(worker) => worker.receiver.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match frame {
                        Some(Ok(message)) => {
                            if self.stream.send(&message).await.is_err() { break; }
                        }
                        Some(Err(())) | None => {
                            self.send_secure_video_error().await;
                            break;
                        }
                    }
                }
                packet = self.stream.next() => {
                    let Some(Ok(bytes)) = packet else { break; };
                    // Upstream decrypt passes very short plaintext through. No valid strict
                    // request needs a decoded body this short.
                    if bytes.len() <= hbb_common::sodiumoxide::crypto::secretbox::MACBYTES {
                        break;
                    }
                    let Ok(message) = Message::parse_from_bytes(&bytes) else { break; };
                    if !self.on_secure_host_message(message).await {
                        break;
                    }
                }
            }
        }
        drop(video_worker);
        self.secure_login_pending = false;
        self.on_close("Secure host session ended", false).await;
    }

    async fn on_secure_host_message(&mut self, message: Message) -> bool {
        match secure_host_policy::classify_message(&message) {
            Request::Login if !self.secure_login_pending && !self.authorized => {
                let Some(message::Union::LoginRequest(lr)) = message.union else {
                    return false;
                };
                if !secure_host_policy::valid_login(&lr, &Config::get_id()) {
                    return false;
                }
                if secure_host_policy::requests_video(&lr) && !secure_video::allowed() {
                    return false;
                }
                if self.require_2fa.is_some() {
                    self.send_login_error(crate::client::REQUIRE_2FA).await;
                    return false;
                }
                self.lr = lr;
                if !self.check_id_whitelist().await {
                    return false;
                }
                self.secure_login_pending = true;
                self.try_start_cm_ipc();
                self.try_start_cm(self.lr.my_id.clone(), self.lr.my_name.clone(), false);
                true
            }
            Request::Heartbeat => {
                if let Some(message::Union::TestDelay(delay)) = message.union {
                    if delay.from_client {
                        let mut reply = Message::new();
                        reply.set_test_delay(delay);
                        return self.stream.send(&reply).await.is_ok();
                    }
                }
                true
            }
            Request::Close => false,
            Request::Login | Request::Denied => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::approve_pending;

    #[test]
    fn cm_approval_requires_this_pending_login_once() {
        let (mut pending, mut authorized) = (false, false);
        assert!(!approve_pending(&mut pending, &mut authorized, false));
        pending = true;
        assert!(!approve_pending(&mut pending, &mut authorized, true));
        assert!(pending && !authorized);
        assert!(approve_pending(&mut pending, &mut authorized, false));
        assert!(!approve_pending(&mut pending, &mut authorized, false));
        assert!(!pending && authorized);
        authorized = false;
        assert!(!approve_pending(&mut pending, &mut authorized, false));
    }
}
