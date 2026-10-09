use std::time::{Duration, Instant};

use serde::{Deserialize, Deserializer};

use crate::protos::message::{OrdVideoSettings, OrdVideoState, PeerInfo};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Settings {
    quality: String,
    fps: u32,
    #[serde(default, deserialize_with = "optional")]
    resolution_mode: Option<String>,
    #[serde(default, deserialize_with = "optional")]
    resolution_width: Option<u32>,
    #[serde(default, deserialize_with = "optional")]
    resolution_height: Option<u32>,
}

pub(crate) fn optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl Settings {
    pub(crate) fn initial(
        quality: Option<String>,
        fps: Option<u32>,
        resolution_mode: Option<String>,
        resolution_width: Option<u32>,
        resolution_height: Option<u32>,
    ) -> Result<Option<Self>, ()> {
        match (quality, fps) {
            (None, None)
                if resolution_mode.is_none()
                    && resolution_width.is_none()
                    && resolution_height.is_none() =>
            {
                Ok(None)
            }
            (Some(quality), Some(fps)) => {
                let settings = Self {
                    quality,
                    fps,
                    resolution_mode,
                    resolution_width,
                    resolution_height,
                };
                (settings.valid() && settings.resolution_mode.as_deref() != Some("sync"))
                    .then_some(Some(settings))
                    .ok_or(())
            }
            _ => Err(()),
        }
    }

    pub(crate) fn parse(raw: &str) -> Result<Self, ()> {
        if raw.len() > 256 {
            return Err(());
        }
        let settings: Self = serde_json::from_str(raw).map_err(|_| ())?;
        (settings.valid() && (settings.version() == 2 || raw.len() <= 128))
            .then_some(settings)
            .ok_or(())
    }

    fn valid(&self) -> bool {
        matches!(self.quality.as_str(), "low" | "balanced" | "high")
            && matches!(self.fps, 10 | 15 | 30)
            && match (
                self.resolution_mode.as_deref(),
                self.resolution_width,
                self.resolution_height,
            ) {
                (None, None, None) => true,
                (Some("preserve"), Some(width), Some(height)) => {
                    matches!(
                        (width, height),
                        (0, 0) | (1280, 720) | (1920, 1080) | (2560, 1440)
                    )
                }
                (Some("sync"), Some(width), Some(height)) => {
                    (width, height) == (0, 0) || stream_dimensions(width, height)
                }
                _ => false,
            }
    }

    fn dimensions(&self, width: u32, height: u32) -> bool {
        if self.resolution_mode.is_some() {
            return stream_dimensions(width, height)
                && (self.resolution_mode.as_deref() != Some("preserve")
                    || self.resolution_width == Some(0)
                    || width <= self.resolution_width.unwrap_or(0)
                        && height <= self.resolution_height.unwrap_or(0));
        }
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
            version: self.version(),
            request_id,
            quality: self.quality.clone(),
            fps: self.fps,
            resolution_mode: self.resolution_mode.clone().unwrap_or_default(),
            resolution_width: self.resolution_width.unwrap_or_default(),
            resolution_height: self.resolution_height.unwrap_or_default(),
            ..Default::default()
        }
    }

    fn version(&self) -> u32 {
        if self.resolution_mode.is_some() {
            2
        } else {
            1
        }
    }
}

fn stream_dimensions(width: u32, height: u32) -> bool {
    (2..=4096).contains(&width)
        && (2..=4096).contains(&height)
        && width % 2 == 0
        && height % 2 == 0
        && u64::from(width) * u64::from(height) <= 8_294_400
}

fn source_dimensions(width: u32, height: u32) -> bool {
    (2..=32768).contains(&width) && (2..=32768).contains(&height)
}

fn same_choice(state: &OrdVideoState, request: &OrdVideoSettings) -> bool {
    state.quality == request.quality
        && state.fps == request.fps
        && state.resolution_mode == request.resolution_mode
        && state.resolution_width == request.resolution_width
        && state.resolution_height == request.resolution_height
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
    applied: Option<OrdVideoState>,
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
            applied: None,
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
    pub(crate) fn version(&self) -> u32 {
        self.initial.as_ref().map_or(0, Settings::version)
    }
    pub(crate) fn resolution_pending(&self) -> bool {
        self.inflight
            .as_ref()
            .is_some_and(|inflight| inflight.request.version == 2)
    }
    pub(crate) fn response_timeout(&self) -> Duration {
        Duration::from_secs(
            if self.inflight.as_ref().is_some_and(|inflight| {
                inflight.request.version == 2
            }) {
                20
            } else {
                5
            },
        )
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
            || fields.get("video_settings_version")?.as_u64()? != u64::from(settings.version())
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
        if settings.version() != self.version() {
            return 2;
        }
        if settings.resolution_mode.as_deref() == Some("sync") {
            let Some(applied) = &self.applied else {
                return 4;
            };
            if !applied.resolution_sync_supported
                || settings.resolution_width != Some(0)
                    && !applied.supported_resolutions.iter().any(|resolution| {
                        Some(resolution.width as u32) == settings.resolution_width
                            && Some(resolution.height as u32) == settings.resolution_height
                    })
            {
                return 2;
            }
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
            resolution_mode: (state.version == 2).then(|| state.resolution_mode.clone()),
            resolution_width: (state.version == 2).then_some(state.resolution_width),
            resolution_height: (state.version == 2).then_some(state.resolution_height),
        };
        if !self.supported
            || !inflight.sent
            || state.version != inflight.request.version
            || state
                .special_fields
                .unknown_fields()
                .iter()
                .next()
                .is_some()
            || state.request_id != inflight.request.request_id
            || !settings.valid()
            || !settings.dimensions(state.width, state.height)
        {
            return Err(());
        }
        if state.version == 1 {
            if !same_choice(state, &inflight.request)
                || state.desktop_width != 0
                || state.desktop_height != 0
                || state.original_width != 0
                || state.original_height != 0
                || !state.supported_resolutions.is_empty()
                || !state.error_code.is_empty()
                || state.resolution_sync_supported
            {
                return Err(());
            }
        } else {
            if !source_dimensions(state.desktop_width, state.desktop_height)
                || !source_dimensions(state.original_width, state.original_height)
                || state.supported_resolutions.len() > 128
                || state.supported_resolutions.iter().any(|resolution| {
                    !stream_dimensions(resolution.width as u32, resolution.height as u32)
                        || resolution
                            .special_fields
                            .unknown_fields()
                            .iter()
                            .next()
                            .is_some()
                })
                || state.width > state.desktop_width
                || state.height > state.desktop_height
            {
                return Err(());
            }
            if state.error_code.is_empty() {
                if !same_choice(state, &inflight.request) {
                    return Err(());
                }
                if state.resolution_mode == "sync" {
                    let (width, height) = if state.resolution_width == 0 {
                        (state.original_width, state.original_height)
                    } else {
                        (state.resolution_width, state.resolution_height)
                    };
                    if !state.resolution_sync_supported
                        || (state.desktop_width, state.desktop_height) != (width, height)
                    {
                        return Err(());
                    }
                }
            } else {
                if !matches!(
                    state.error_code.as_str(),
                    "DISPLAY_BUSY"
                        | "UNSUPPORTED_RESOLUTION"
                        | "DISPLAY_CHANGED"
                        | "DISPLAY_SWITCH_FAILED"
                        | "VIDEO_REBUILD_FAILED"
                        | "DISPLAY_CONTROL_REQUIRED"
                ) {
                    return Err(());
                }
                let Some(applied) = &self.applied else {
                    return Err(());
                };
                if state.quality != applied.quality
                    || state.fps != applied.fps
                    || state.resolution_mode != applied.resolution_mode
                    || state.resolution_width != applied.resolution_width
                    || state.resolution_height != applied.resolution_height
                {
                    return Err(());
                }
            }
        }
        self.applied = Some(state.clone());
        self.inflight = None;
        self.ready = true;
        Ok(())
    }

    pub(crate) fn timed_out(&self) -> bool {
        self.inflight
            .as_ref()
            .is_some_and(|request| request.started.elapsed() >= self.response_timeout())
    }

    pub(crate) fn clear(&mut self) {
        self.supported = false;
        self.ready = false;
        self.inflight = None;
        self.outbound = None;
        self.applied = None;
    }
}
