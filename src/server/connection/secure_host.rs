use super::super::secure_host_policy::{self, Request};
use super::super::{secure_access, secure_access_policy, secure_input, secure_video};
use super::*;

fn approve_pending(pending: &mut bool, authorized: &mut bool, requires_2fa: bool) -> bool {
    if !*pending || *authorized || requires_2fa {
        return false;
    }
    *pending = false;
    *authorized = true;
    true
}

fn local_approval_allowed() -> bool {
    get_time().saturating_sub(CLICK_TIME.load(Ordering::SeqCst)) > 200
}

impl Connection {
    async fn send_secure_input_state(&mut self, input: &secure_input::Control, request_id: u64) -> bool {
        self.keyboard = input.enabled();
        self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_input_supported".into(), enabled: input.supported() });
        self.send_to_cm(ipc::Data::SwitchPermission { name: "keyboard".into(), enabled: self.keyboard });
        if self.stream.send(&input.state(request_id)).await.is_err() { return false; }
        let mut misc = Misc::new();
        misc.set_permission_info(PermissionInfo { permission: Permission::Keyboard.into(), enabled: self.keyboard, ..Default::default() });
        let mut message = Message::new();
        message.set_misc(misc);
        self.stream.send(&message).await.is_ok()
    }

    async fn send_secure_input_error(&mut self, reason: &str) {
        let mut misc = Misc::new();
        misc.set_close_reason(reason.to_owned());
        let mut message = Message::new();
        message.set_misc(misc);
        if self.stream.send(&message).await.is_err() {
            log::trace!("Secure input close notification could not be sent");
        }
    }
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
        self.hash.challenge = secure_access::challenge();
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
        let mut video_settings_id = 0;
        let mut input = secure_input::Control::new(self.inner.id());
        let mut access_pending: Option<base::message_proto::OrdAccessRequest> = None;
        let mut access_active: Option<String> = None;
        let mut access_deadline = started + Duration::from_secs(15 * 60);
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
                    if let Some(id) = access_active.clone() {
                        if Instant::now() >= access_deadline { break; }
                        if !matches!(tokio::task::spawn_blocking(move || secure_access::active(&id)).await, Ok(Ok(()))) {
                            self.send_login_error("Trusted access expired, revoked, or policy changed").await;
                            break;
                        }
                    }
                    match input.tick() {
                        Ok(true) => {
                            self.send_secure_input_state(&input, 0).await;
                            break;
                        }
                        Err(error) => { log::warn!("Secure input watchdog stopped: {error}"); break; }
                        Ok(false) => {}
                    }
                }
                _ = time::sleep_until(session_deadline) => break,
                _ = time::sleep_until(login_deadline), if !self.authorized => break,
                _ = unauthorized_evicted(&self.unauthorized_id) => break,
                data = rx_from_cm.recv() => {
                    let Some(data) = data else { break; };
                    match data {
                        ipc::Data::Authorize if access_pending.is_some() => {
                            self.send_login_error("Pairing requires explicit local registration approval").await;
                            break;
                        }
                        ipc::Data::SwitchPermission { name, enabled: true } if name == "ord_pair_device" => {
                            if !secure_access_policy::pair_approval_allowed(self.secure_login_pending, self.authorized, access_pending.is_some(), local_approval_allowed()) || self.require_2fa.is_some() { break; }
                            let Some(request) = access_pending.take() else { break; };
                            let challenge = self.hash.challenge.clone();
                            let name = self.lr.my_name.clone();
                            let result = tokio::task::spawn_blocking(move || secure_access::approve_pair(&request, &challenge, &name)).await;
                            let Ok(Ok((grant, record))) = result else {
                                self.send_login_error("Trusted device registration failed").await;
                                break;
                            };
                            let mut response = Message::new(); response.set_ord_access_grant(grant);
                            let sent = self.stream.send(&response).await.is_ok();
                            if let Some(message::Union::OrdAccessGrant(grant)) = response.union.take() {
                                if let Ok(mut secret) = grant.credential_secret.try_into_mut() { hbb_common::sodiumoxide::utils::memzero(&mut secret); }
                            }
                            if !sent { break; }
                            let Some(worker) = self.start_secure_access_video("pair").await else { break; };
                            video_worker = Some(worker);
                            access_active = Some(record.id);
                            access_deadline = Instant::now() + Duration::from_secs(15 * 60);
                            self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_access_pair_pending".into(), enabled: false });
                        }
                        ipc::Data::Authorize if local_approval_allowed() && approve_pending(
                            &mut self.secure_login_pending,
                            &mut self.authorized,
                            self.require_2fa.is_some(),
                        ) => {
                            self.unauthorized_id = None;
                            self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::Click);
                            let video_requested = secure_host_policy::requests_video(&self.lr);
                            let input_requested = secure_host_policy::requests_input(&self.lr);
                            let video_settings = secure_host_policy::initial_video_settings(&self.lr);
                            if video_requested && !secure_video::allowed() {
                                self.send_secure_video_error().await;
                                break;
                            }
                            let video_dimensions = if video_requested {
                                let result = if let Some(settings) = video_settings {
                                    tokio::task::spawn_blocking(move || secure_video::dimensions_with_settings(settings)).await
                                } else {
                                    tokio::task::spawn_blocking(secure_video::dimensions).await
                                };
                                let Ok(Ok(dimensions)) = result else {
                                    self.send_secure_video_error().await;
                                    break;
                                };
                                Some(dimensions)
                            } else {
                                None
                            };
                            let info = if let Some(dimensions) = video_dimensions {
                                if video_settings.is_some() {
                                    secure_host_policy::approved_configurable_video_peer_info(VERSION, dimensions.width as _, dimensions.height as _)
                                } else if input_requested {
                                    secure_host_policy::approved_control_peer_info(VERSION, dimensions.width as _, dimensions.height as _)
                                } else {
                                    secure_host_policy::approved_video_peer_info(VERSION, dimensions.width as _, dimensions.height as _)
                                }
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
                                let worker = if let Some(settings) = video_settings {
                                    secure_video::start_control_with_settings(dimensions, settings)
                                } else if input_requested { secure_video::start_control(dimensions) } else { secure_video::start(dimensions) };
                                match worker {
                                    Ok(worker) => {
                                        if let Some(settings) = video_settings {
                                            if self.stream.send(&settings.state(dimensions.width, dimensions.height)).await.is_err() { break; }
                                            video_settings_id = settings.request_id;
                                        }
                                        video_worker = Some(worker);
                                    }
                                    Err(_) => {
                                        self.send_secure_video_error().await;
                                        break;
                                    }
                                }
                            }
                            #[cfg(feature = "flutter")]
                            self.try_start_cm(self.lr.my_id.clone(), self.lr.my_name.clone(), true);
                            if input_requested {
                                input.configure(Self::permission(keys::OPTION_ENABLE_KEYBOARD, &self.control_permissions));
                                if !self.send_secure_input_state(&input, 0).await { break; }
                            }
                        }
                        ipc::Data::Close => break,
                        ipc::Data::CmErr(_) => break,
                        ipc::Data::SwitchPermission { .. } => {
                            if self.authorized && secure_host_policy::requests_input(&self.lr) {
                                self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_input_supported".into(), enabled: input.supported() });
                                self.send_to_cm(ipc::Data::SwitchPermission { name: "keyboard".into(), enabled: input.enabled() });
                            } else if !self.send_secure_permissions().await {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                Some(_) = rx.recv() => {}
                Some(_) = rx_video.recv() => {}
                Some(_) = rx_from_authed.recv() => {}
                // Control and settings must remain responsive when the video queue stays full.
                packet = self.stream.next() => {
                    let Some(Ok(bytes)) = packet else { break; };
                    // Upstream decrypt passes very short plaintext through. No valid strict
                    // request needs a decoded body this short.
                    if bytes.len() <= hbb_common::sodiumoxide::crypto::secretbox::MACBYTES {
                        break;
                    }
                    let Ok(message) = Message::parse_from_bytes(&bytes) else { break; };
                    if secure_host_policy::classify_message(&message) == Request::VideoSettings {
                        let Some(message::Union::OrdVideoSettings(request)) = message.union else { break; };
                        let Some(settings) = secure_host_policy::updated_video_settings(
                            &self.lr, self.authorized, video_settings_id, &request,
                        ) else { break; };
                        // The old receiver must be gone before acknowledging a new frame size.
                        drop(video_worker.take());
                        let Ok(Ok(dimensions)) = tokio::task::spawn_blocking(move || secure_video::dimensions_with_settings(settings)).await else {
                            self.send_secure_video_error().await;
                            break;
                        };
                        let Ok(worker) = secure_video::start_control_with_settings(dimensions, settings) else {
                            self.send_secure_video_error().await;
                            break;
                        };
                        if self.stream.send(&settings.state(dimensions.width, dimensions.height)).await.is_err() { break; }
                        video_settings_id = settings.request_id;
                        video_worker = Some(worker);
                        continue;
                    }
                    if secure_host_policy::classify_message(&message) == Request::InputRequest {
                        if !self.authorized || !secure_host_policy::requests_input(&self.lr) { break; }
                        let Some(message::Union::OrdInputRequest(request)) = message.union else { break; };
                        if let Err(error) = input.request_enabled(request.enabled) {
                            log::trace!("Secure input request was not applied: {error}");
                        }
                        if input.poisoned() {
                            self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_input_release_failed".into(), enabled: true });
                        }
                        if !self.send_secure_input_state(&input, request.request_id).await || input.poisoned() { break; }
                        continue;
                    }
                    if secure_host_policy::classify_message(&message) == Request::Input {
                        if !self.authorized || !secure_host_policy::requests_input(&self.lr) { break; }
                        let Some(message::Union::OrdInputEvent(event)) = message.union else { break; };
                        let echo = matches!(&event.command, Some(ord_input_event::Command::KeepAlive(_))).then(|| event.clone());
                        match input.apply(event) {
                            Ok(true) => {
                                if let Some(echo) = echo {
                                    let mut reply = Message::new();
                                    reply.set_ord_input_event(echo);
                                    if self.stream.send(&reply).await.is_err() { break; }
                                }
                            }
                            Ok(false) => {}
                            Err(error) => { log::warn!("Secure input rejected: {error}"); break; }
                        }
                        continue;
                    }
                    if let Some(message::Union::LoginRequest(lr)) = &message.union {
                        if let Some(request) = lr.ord_access.as_ref() {
                            if secure_host_policy::classify_message(&message) != Request::Login || self.authorized || self.secure_login_pending || !secure_host_policy::valid_login(lr, &Config::get_id())
                                || !secure_host_policy::requests_video(lr) || secure_host_policy::requests_input(lr) || !secure_video::allowed() || self.require_2fa.is_some() {
                                self.send_login_error("Trusted access requires enabled read-only video and no 2FA").await;
                                break;
                            }
                            let request = request.clone();
                            let verification = request.clone();
                            let challenge = self.hash.challenge.clone();
                            let result = tokio::task::spawn_blocking(move || secure_access::prepare(&verification, &challenge)).await;
                            let Ok(Ok(record)) = result else {
                                self.send_login_error("Trusted controller proof or credential rejected").await;
                                break;
                            };
                            self.lr = lr.clone();
                            // The verified secret is not needed for later revocation checks.
                            self.lr.ord_access = None.into();
                            if !self.check_id_whitelist().await { break; }
                            self.secure_login_pending = true;
                            self.try_start_cm_ipc();
                            let label = format!("{} [SHA256 {}]", self.lr.my_name, secure_access::fingerprint(&request));
                            self.try_start_cm(self.lr.my_id.clone(), label, false);
                            if let Some(record) = record {
                                let Some(worker) = self.start_secure_access_video("unattended").await else { break; };
                                video_worker = Some(worker);
                                access_active = Some(record.id);
                                access_deadline = Instant::now() + Duration::from_secs(15 * 60);
                                self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_access_unattended".into(), enabled: true });
                            } else {
                                access_pending = Some(request);
                                self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_access_pair_pending".into(), enabled: true });
                            }
                            continue;
                        }
                    }
                    if !self.on_secure_host_message(message).await {
                        break;
                    }
                }
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
            }
        }
        if let Err(error) = input.revoke() {
            self.send_to_cm(ipc::Data::SwitchPermission { name: "ord_input_release_failed".into(), enabled: true });
            self.keyboard = false;
            log::error!("Secure input cleanup failed: {error}");
            if secure_host_policy::requests_input(&self.lr) {
                self.send_secure_input_state(&input, 0).await;
                self.send_secure_input_error("Input release failed").await;
            }
        } else if self.authorized && secure_host_policy::requests_input(&self.lr) {
            self.send_secure_input_state(&input, 0).await;
        }
        drop(video_worker);
        self.secure_login_pending = false;
        self.on_close("Secure host session ended", false).await;
    }

    async fn start_secure_access_video(&mut self, mode: &str) -> Option<secure_video::Worker> {
        if !secure_video::allowed() || super::super::secure_host::is_stopped() { return None; }
        let Ok(Ok(dimensions)) = tokio::task::spawn_blocking(secure_video::dimensions).await else {
            self.send_secure_video_error().await; return None;
        };
        let mut info = secure_host_policy::approved_video_peer_info(VERSION, dimensions.width as _, dimensions.height as _);
        if let Some(message::Union::LoginResponse(login)) = info.union.as_mut() {
            if let Some(base::message_proto::login_response::Union::PeerInfo(peer)) = login.union.as_mut() {
                let Ok(mut additions) = serde_json::from_str::<serde_json::Value>(&peer.platform_additions) else { return None; };
                additions["access_mode"] = mode.into();
                peer.platform_additions = additions.to_string();
            }
        }
        if self.stream.send(&info).await.is_err() || !self.send_secure_permissions().await { return None; }
        let Ok(worker) = secure_video::start(dimensions) else { self.send_secure_video_error().await; return None; };
        self.authorized = true;
        self.secure_login_pending = false;
        self.unauthorized_id = None;
        self.try_start_cm(self.lr.my_id.clone(), self.lr.my_name.clone(), true);
        Some(worker)
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
            Request::Login | Request::Input | Request::InputRequest | Request::VideoSettings | Request::Denied => false,
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
