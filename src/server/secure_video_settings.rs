use base::message_proto::{Message, OrdVideoSettings, OrdVideoState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub request_id: u64,
    pub quality: &'static str,
    pub fps: u32,
}

impl Settings {
    pub fn parse(request: &OrdVideoSettings, previous_id: u64) -> Option<Self> {
        if request.version != 1
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
        Some(Self {
            request_id: request.request_id,
            quality,
            fps: request.fps,
        })
    }

    pub fn dimensions(self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width < 2 || height < 2 {
            return None;
        }
        let (max_width, max_height) = match self.quality {
            "low" => (1280, 720),
            "balanced" => (1920, 1080),
            "high" => (2560, 1440),
            _ => return None,
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
}
