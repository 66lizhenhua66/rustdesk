use std::time::{Duration, Instant};

use serde::{Deserialize, Deserializer};

use crate::protos::message::{OrdVideoSettings, OrdVideoState, PeerInfo};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    quality: String,
    fps: u32,
}

pub(crate) fn optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl Settings {
    pub(crate) fn initial(quality: Option<String>, fps: Option<u32>) -> Result<Option<Self>, ()> {
        match (quality, fps) {
            (None, None) => Ok(None),
            (Some(quality), Some(fps)) => {
                let settings = Self { quality, fps };
                settings.valid().then_some(Some(settings)).ok_or(())
            }
            _ => Err(()),
        }
    }

    pub(crate) fn parse(raw: &str) -> Result<Self, ()> {
        if raw.len() > 128 {
            return Err(());
        }
        let settings: Self = serde_json::from_str(raw).map_err(|_| ())?;
        settings.valid().then_some(settings).ok_or(())
    }

    fn valid(&self) -> bool {
        matches!(self.quality.as_str(), "low" | "balanced" | "high")
            && matches!(self.fps, 10 | 15 | 30)
    }

    fn dimensions(&self, width: u32, height: u32) -> bool {
        let (max_width, max_height) = match self.quality.as_str() {
            "low" => (1280, 720),
            "balanced" => (1920, 1080),
            "high" => (2560, 1440),
            _ => return false,
        };
        (2..=max_width).contains(&width)
            && (2..=max_height).contains(&height)
            && width % 2 == 0
            && height % 2 == 0
    }

    fn request(&self, request_id: u64) -> OrdVideoSettings {
        OrdVideoSettings {
            version: 1,
            request_id,
            quality: self.quality.clone(),
            fps: self.fps,
            ..Default::default()
        }
    }
}

struct Inflight {
    request: OrdVideoSettings,
    started: Instant,
    sent: bool,
}

pub(crate) struct VideoSettings {
    initial: Option<Settings>,
    supported: bool,
    ready: bool,
    next_id: u64,
    inflight: Option<Inflight>,
    outbound: Option<OrdVideoSettings>,
}

impl VideoSettings {
    pub(crate) fn new(initial: Option<Settings>) -> Self {
        Self {
            initial,
            supported: false,
            ready: false,
            next_id: 2,
            inflight: None,
            outbound: None,
        }
    }

    pub(crate) fn requested(&self) -> bool {
        self.initial.is_some()
    }
    pub(crate) fn supported(&self) -> bool {
        self.supported
    }
    pub(crate) fn ready(&self) -> bool {
        !self.requested() || self.ready
    }
    pub(crate) fn initial_request(&self) -> Option<OrdVideoSettings> {
        self.initial.as_ref().map(|settings| settings.request(1))
    }

    pub(crate) fn negotiate(&mut self, peer: &PeerInfo) -> Option<(u32, u32)> {
        let settings = self.initial.as_ref()?;
        let value: serde_json::Value = serde_json::from_str(&peer.platform_additions).ok()?;
        let fields = value.as_object()?;
        if fields.len() != 6
            || fields.get("ord_secure_host")?.as_u64()? != 1
            || fields.get("media")?.as_bool()? != true
            || fields.get("video_codec")?.as_str()? != "vp8"
            || fields.get("input_scope")?.as_str()? != "windows_primary"
            || fields.get("input_version")?.as_u64()? != 2
            || fields.get("video_settings_version")?.as_u64()? != 1
            || peer.displays.len() != 1
            || peer.current_display != 0
            || !peer.displays[0].online
        {
            return None;
        }
        let display = &peer.displays[0];
        let width = u32::try_from(display.width).ok()?;
        let height = u32::try_from(display.height).ok()?;
        if !settings.dimensions(width, height) {
            return None;
        }
        self.inflight = Some(Inflight {
            request: settings.request(1),
            started: Instant::now(),
            sent: true,
        });
        self.supported = true;
        Some((width, height))
    }

    pub(crate) fn queue(&mut self, settings: Settings) -> i32 {
        if !self.supported {
            return 2;
        }
        if self.inflight.is_some() {
            return 4;
        }
        let Some(next_id) = self.next_id.checked_add(1) else {
            return 4;
        };
        let request = settings.request(self.next_id);
        self.next_id = next_id;
        self.inflight = Some(Inflight {
            request: request.clone(),
            started: Instant::now(),
            sent: false,
        });
        self.outbound = Some(request);
        0
    }

    pub(crate) fn take_request(&mut self) -> Option<OrdVideoSettings> {
        let request = self.outbound.take()?;
        if let Some(inflight) = self.inflight.as_mut() {
            inflight.sent = true;
        }
        Some(request)
    }

    pub(crate) fn apply(&mut self, state: &OrdVideoState) -> Result<(), ()> {
        let Some(inflight) = &self.inflight else {
            return Err(());
        };
        let settings = Settings {
            quality: state.quality.clone(),
            fps: state.fps,
        };
        if !self.supported
            || !inflight.sent
            || state.version != 1
            || state
                .special_fields
                .unknown_fields()
                .iter()
                .next()
                .is_some()
            || state.request_id != inflight.request.request_id
            || state.quality != inflight.request.quality
            || state.fps != inflight.request.fps
            || !settings.dimensions(state.width, state.height)
        {
            return Err(());
        }
        self.inflight = None;
        self.ready = true;
        Ok(())
    }

    pub(crate) fn timed_out(&self) -> bool {
        self.inflight
            .as_ref()
            .is_some_and(|request| request.started.elapsed() >= Duration::from_secs(5))
    }

    pub(crate) fn clear(&mut self) {
        self.supported = false;
        self.ready = false;
        self.inflight = None;
        self.outbound = None;
    }
}
