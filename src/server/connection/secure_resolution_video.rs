use super::super::super::{
    secure_display::{DisplayError, DisplaySession, DisplaySnapshot},
    secure_host_policy::video_settings::{valid_sync_dimensions, DesktopState, Settings},
    secure_input,
    secure_video::{self, Dimensions, Worker},
};
use base::message_proto::Message;
use hbb_common::{bail, log, tokio::task::spawn_blocking, ResultType};

pub(super) struct ResolutionVideo {
    display: Option<DisplaySession>,
    desktop: DesktopState,
    settings: Settings,
    dimensions: Dimensions,
}

fn desktop_state(snapshot: DisplaySnapshot) -> DesktopState {
    DesktopState {
        width: snapshot.current_width,
        height: snapshot.current_height,
        original_width: snapshot.original_width,
        original_height: snapshot.original_height,
        modes: snapshot
            .modes
            .into_iter()
            .filter(|mode| valid_sync_dimensions(mode.width, mode.height))
            .map(|mode| (mode.width, mode.height))
            .collect(),
        sync_supported: true,
    }
}

fn error_code(error: &DisplayError) -> &'static str {
    match error {
        DisplayError::Busy => "DISPLAY_BUSY",
        DisplayError::UnsupportedMode => "UNSUPPORTED_RESOLUTION",
        DisplayError::ExternalChanged => "DISPLAY_CHANGED",
        DisplayError::Failed(_) => "DISPLAY_SWITCH_FAILED",
        DisplayError::RecoveryFailed(_) => "DISPLAY_SWITCH_FAILED",
    }
}

impl ResolutionVideo {
    pub(super) async fn open(settings: Settings) -> ResultType<Self> {
        let result = spawn_blocking(|| {
            let mut display = DisplaySession::new()?;
            let snapshot = display.snapshot()?;
            Ok::<_, DisplayError>((display, snapshot))
        })
        .await?;
        let (display, desktop) = match result {
            Ok((display, snapshot)) => (Some(display), desktop_state(snapshot)),
            Err(DisplayError::RecoveryFailed(error)) => bail!("Display recovery failed: {error}"),
            Err(error) => {
                log::trace!("Desktop synchronization is unavailable: {error}");
                let current = spawn_blocking(secure_video::primary_dimensions).await??;
                (
                    None,
                    DesktopState {
                        width: current.width,
                        height: current.height,
                        original_width: current.width,
                        original_height: current.height,
                        modes: Vec::new(),
                        sync_supported: false,
                    },
                )
            }
        };
        let Some((width, height)) = settings.dimensions(desktop.width, desktop.height) else {
            bail!("Desktop dimensions cannot be encoded");
        };
        Ok(Self {
            display,
            desktop,
            settings,
            dimensions: Dimensions { width, height },
        })
    }

    pub(super) fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    pub(super) async fn start(&mut self) -> ResultType<(Worker, Message, Message)> {
        let (worker, first, dimensions, desktop) = rebuild(self.settings).await?;
        self.dimensions = dimensions;
        self.desktop.width = desktop.width;
        self.desktop.height = desktop.height;
        Ok((
            worker,
            self.settings.state_with_desktop(
                dimensions.width,
                dimensions.height,
                &self.desktop,
                "",
            ),
            first,
        ))
    }

    async fn apply_desktop(
        &mut self,
        width: u32,
        height: u32,
        require_input: bool,
    ) -> Result<bool, DisplayError> {
        let Some(mut display) = self.display.take() else {
            return Err(DisplayError::Busy);
        };
        let (display, result) = spawn_blocking(move || {
            let result = if require_input && !secure_input::Control::display_capability_allowed() {
                Ok(None)
            } else {
                display.apply(width, height).map(Some)
            };
            (display, result)
        })
        .await
        .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
        self.display = Some(display);
        let Some(snapshot) = result? else {
            return Ok(false);
        };
        self.desktop = desktop_state(snapshot);
        Ok(true)
    }

    pub(super) async fn update(
        &mut self,
        worker: &mut Option<Worker>,
        requested: Settings,
        input_enabled: bool,
    ) -> ResultType<(Message, Message)> {
        if let Some(old) = worker.take() {
            old.stop().await?;
        }
        let previous_settings = self.settings;
        let previous_desktop = self.desktop.clone();
        let mut failure = "";
        let mut rollback_required = false;
        if requested.resolution_mode == "sync" && !input_enabled {
            failure = "DISPLAY_CONTROL_REQUIRED";
        } else if requested.resolution_mode == "sync" && !self.desktop.sync_supported {
            failure = "DISPLAY_BUSY";
        } else {
            let mode = if requested.resolution_mode == "sync" {
                Some((requested.resolution_width, requested.resolution_height))
            } else if previous_settings.resolution_mode == "sync" && self.desktop.sync_supported {
                Some((0, 0))
            } else {
                None
            };
            if let Some((width, height)) = mode {
                match self
                    .apply_desktop(width, height, requested.resolution_mode == "sync")
                    .await
                {
                    Ok(true) => rollback_required = true,
                    Ok(false) => failure = "DISPLAY_CONTROL_REQUIRED",
                    Err(error) => {
                        if matches!(error, DisplayError::RecoveryFailed(_)) {
                            bail!("Display rollback failed: {error}");
                        }
                        if matches!(error, DisplayError::ExternalChanged) {
                            self.desktop.sync_supported = false;
                            self.desktop.modes.clear();
                        }
                        rollback_required = !matches!(error, DisplayError::ExternalChanged);
                        failure = error_code(&error);
                    }
                }
            }
        }
        if failure.is_empty() {
            match rebuild(requested).await {
                Ok((next, first, dimensions, desktop)) => {
                    self.settings = requested;
                    self.dimensions = dimensions;
                    self.desktop.width = desktop.width;
                    self.desktop.height = desktop.height;
                    *worker = Some(next);
                    return Ok((
                        requested.state_with_desktop(
                            dimensions.width,
                            dimensions.height,
                            &self.desktop,
                            "",
                        ),
                        first,
                    ));
                }
                Err(error) => {
                    log::trace!("Secure video rebuild failed: {error}");
                    failure = "VIDEO_REBUILD_FAILED";
                }
            }
        }
        if rollback_required {
            let (width, height) = if previous_desktop.width == previous_desktop.original_width
                && previous_desktop.height == previous_desktop.original_height
            {
                (0, 0)
            } else {
                (previous_desktop.width, previous_desktop.height)
            };
            if let Err(error) = self.apply_desktop(width, height, false).await {
                if matches!(error, DisplayError::ExternalChanged) {
                    failure = "DISPLAY_CHANGED";
                    self.desktop.sync_supported = false;
                    self.desktop.modes.clear();
                } else {
                    bail!("Display rollback failed: {error}");
                }
            }
        }
        let (next, first, dimensions, desktop) = rebuild(previous_settings).await?;
        self.settings = Settings {
            request_id: requested.request_id,
            ..previous_settings
        };
        self.dimensions = dimensions;
        self.desktop.width = desktop.width;
        self.desktop.height = desktop.height;
        *worker = Some(next);
        Ok((
            self.settings.state_with_desktop(
                dimensions.width,
                dimensions.height,
                &self.desktop,
                failure,
            ),
            first,
        ))
    }

    pub(super) async fn restore(mut self) -> ResultType<()> {
        if let Some(mut display) = self.display.take() {
            let result = spawn_blocking(move || display.restore()).await?;
            if let Err(error) = result {
                if matches!(error, DisplayError::ExternalChanged) {
                    log::warn!(
                        "Secure display was changed locally; preserving the current desktop"
                    );
                } else {
                    bail!("Secure display restoration failed: {error}");
                }
            }
        }
        Ok(())
    }
}

async fn rebuild(settings: Settings) -> ResultType<(Worker, Message, Dimensions, Dimensions)> {
    let desktop = spawn_blocking(secure_video::primary_dimensions).await??;
    let Some((width, height)) = settings.dimensions(desktop.width, desktop.height) else {
        bail!("Desktop dimensions cannot be encoded");
    };
    let dimensions = Dimensions { width, height };
    let mut worker = secure_video::start_control_with_settings(dimensions, settings)?;
    match worker.first_frame(dimensions).await {
        Ok(first) => Ok((worker, first, dimensions, desktop)),
        Err(error) => {
            worker.stop().await?;
            Err(error)
        }
    }
}
