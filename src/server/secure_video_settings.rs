use base::message_proto::{Message, OrdVideoSettings, OrdVideoState, Resolution};

#[derive(Clone, Debug)]
pub struct DesktopState {
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    pub modes: Vec<(u32, u32)>,
    pub sync_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub version: u32,
    pub request_id: u64,
    pub quality: &'static str,
    pub fps: u32,
    pub resolution_mode: &'static str,
    pub resolution_width: u32,
    pub resolution_height: u32,
}

impl Settings {
    pub fn parse(request: &OrdVideoSettings, previous_id: u64) -> Option<Self> {
        if !matches!(request.version, 1 | 2)
            || request.request_id <= previous_id
            || !matches!(request.fps, 10 | 15 | 30)
            || request
                .special_fields
                .unknown_fields()
                .iter()
                .next()
                .is_some()
        {
            return None;
        }
        let quality = match request.quality.as_str() {
            "low" => "low",
            "balanced" => "balanced",
            "high" => "high",
            _ => return None,
        };
        let resolution_mode = match (request.version, request.resolution_mode.as_str()) {
            (1, "") if request.resolution_width == 0 && request.resolution_height == 0 => "",
            (2, "preserve")
                if matches!(
                    (request.resolution_width, request.resolution_height),
                    (0, 0) | (1280, 720) | (1920, 1080) | (2560, 1440)
                ) =>
            {
                "preserve"
            }
            (2, "sync")
                if (request.resolution_width == 0 && request.resolution_height == 0)
                    || valid_sync_dimensions(
                        request.resolution_width,
                        request.resolution_height,
                    ) =>
            {
                "sync"
            }
            _ => return None,
        };
        Some(Self {
            version: request.version,
            request_id: request.request_id,
            quality,
            fps: request.fps,
            resolution_mode,
            resolution_width: request.resolution_width,
            resolution_height: request.resolution_height,
        })
    }

    pub fn dimensions(self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width < 2 || height < 2 {
            return None;
        }
        let (max_width, max_height) = if self.version == 2 {
            if self.resolution_mode == "preserve" && self.resolution_width != 0 {
                (self.resolution_width, self.resolution_height)
            } else {
                let scale = 1.0f64
                    .min(4096.0 / f64::from(width.max(height)))
                    .min((8294400.0 / (f64::from(width) * f64::from(height))).sqrt());
                let width = (f64::from(width) * scale) as u32 & !1;
                let height = (f64::from(height) * scale) as u32 & !1;
                return (width >= 2 && height >= 2).then_some((width, height));
            }
        } else {
            match self.quality {
                "low" => (1280, 720),
                "balanced" => (1920, 1080),
                "high" => (2560, 1440),
                _ => return None,
            }
        };
        let (width, height) = if width <= max_width && height <= max_height {
            (width, height)
        } else if u64::from(width) * u64::from(max_height)
            > u64::from(height) * u64::from(max_width)
        {
            (
                max_width,
                (u64::from(height) * u64::from(max_width) / u64::from(width)) as u32,
            )
        } else {
            (
                (u64::from(width) * u64::from(max_height) / u64::from(height)) as u32,
                max_height,
            )
        };
        let (width, height) = (width & !1, height & !1);
        (width >= 2 && height >= 2).then_some((width, height))
    }

    pub fn encoder_quality(self) -> f32 {
        match self.quality {
            "high" => 2.0,
            "balanced" => 1.5,
            _ => 1.0,
        }
    }

    pub fn state(self, width: u32, height: u32) -> Message {
        let mut message = Message::new();
        message.set_ord_video_state(OrdVideoState {
            version: 1,
            request_id: self.request_id,
            quality: self.quality.into(),
            fps: self.fps,
            width,
            height,
            ..Default::default()
        });
        message
    }

    pub fn state_with_desktop(
        self,
        width: u32,
        height: u32,
        desktop: &DesktopState,
        error: &str,
    ) -> Message {
        let mut message = Message::new();
        message.set_ord_video_state(OrdVideoState {
            version: 2,
            request_id: self.request_id,
            quality: self.quality.into(),
            fps: self.fps,
            width,
            height,
            resolution_mode: self.resolution_mode.into(),
            resolution_width: self.resolution_width,
            resolution_height: self.resolution_height,
            desktop_width: desktop.width,
            desktop_height: desktop.height,
            original_width: desktop.original_width,
            original_height: desktop.original_height,
            supported_resolutions: desktop
                .modes
                .iter()
                .map(|&(width, height)| Resolution {
                    width: width as i32,
                    height: height as i32,
                    ..Default::default()
                })
                .collect(),
            resolution_sync_supported: desktop.sync_supported,
            error_code: error.into(),
            ..Default::default()
        });
        message
    }
}

pub fn valid_sync_dimensions(width: u32, height: u32) -> bool {
    (2..=4096).contains(&width)
        && (2..=4096).contains(&height)
        && width % 2 == 0
        && height % 2 == 0
        && u64::from(width) * u64::from(height) <= 3840 * 2160
}
