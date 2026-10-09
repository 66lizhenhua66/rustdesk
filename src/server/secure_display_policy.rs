use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub frequency: u32,
    pub bits: u32,
    pub orientation: u32,
    pub fixed_output: u32,
    pub display_flags: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub device: String,
    pub topology: Vec<String>,
    pub mode: Mode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub version: u32,
    pub owner: String,
    pub original: State,
    pub previous: Mode,
    pub requested: Mode,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DisplayError {
    Busy,
    UnsupportedMode,
    ExternalChanged,
    Failed(String),
    RecoveryFailed(String),
}

impl DisplayError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Busy => "displaybusy",
            Self::UnsupportedMode => "unsupportedmode",
            Self::ExternalChanged => "externalchanged",
            Self::Failed(_) => "displayfailed",
            Self::RecoveryFailed(_) => "recoveryfailed",
        }
    }
}

impl fmt::Display for DisplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(detail) | Self::RecoveryFailed(detail) => {
                write!(formatter, "{}: {}", self.code(), detail)
            }
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for DisplayError {}

#[derive(Debug)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct DisplaySnapshot {
    pub original_width: u32,
    pub original_height: u32,
    pub current_width: u32,
    pub current_height: u32,
    pub modes: Vec<DisplayMode>,
}

pub trait Backend {
    fn current(&mut self) -> Result<State, DisplayError>;
    fn capture_original(&mut self) -> Result<State, DisplayError> {
        self.current()
    }
    fn modes(&mut self) -> Result<Vec<Mode>, DisplayError>;
    fn test(&mut self, mode: &Mode) -> Result<(), DisplayError>;
    fn apply(&mut self, mode: &Mode) -> Result<(), DisplayError>;
}

pub trait JournalStore {
    fn read(&mut self) -> Result<Option<Journal>, DisplayError>;
    fn write(&mut self, journal: &Journal) -> Result<(), DisplayError>;
    fn remove(&mut self, owner: &str) -> Result<(), DisplayError>;
    fn report_restore_error(&self, error: &DisplayError);
}

pub struct ModeSession<B: Backend, J: JournalStore> {
    backend: B,
    journal: J,
    original: State,
    last: State,
    owner: String,
    journal_active: bool,
    _exclusive: Exclusive,
}

struct Exclusive(Arc<AtomicBool>);

impl Exclusive {
    fn acquire(exclusive: Arc<AtomicBool>) -> Result<Self, DisplayError> {
        exclusive
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| DisplayError::Busy)?;
        Ok(Self(exclusive))
    }
}

impl Drop for Exclusive {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl<B: Backend, J: JournalStore> ModeSession<B, J> {
    pub fn open(
        mut backend: B,
        mut journal: J,
        exclusive: Arc<AtomicBool>,
        owner: String,
    ) -> Result<Self, DisplayError> {
        let exclusive = Exclusive::acquire(exclusive)?;
        if let Some(pending) = journal.read()? {
            recover(&mut backend, &mut journal, &pending)?;
        }
        let original = backend.capture_original()?;
        Ok(Self {
            backend,
            journal,
            last: original.clone(),
            original,
            owner,
            journal_active: false,
            _exclusive: exclusive,
        })
    }

    pub fn snapshot(&mut self) -> Result<DisplaySnapshot, DisplayError> {
        let current = self.unchanged()?;
        let mut sizes: Vec<_> = self
            .backend
            .modes()?
            .into_iter()
            .filter(|mode| compatible(mode, &current.mode))
            .map(|mode| (mode.width, mode.height))
            .collect();
        sizes.sort_unstable();
        sizes.dedup();
        sizes.truncate(128);
        Ok(DisplaySnapshot {
            original_width: self.original.mode.width,
            original_height: self.original.mode.height,
            current_width: current.mode.width,
            current_height: current.mode.height,
            modes: sizes
                .into_iter()
                .map(|(width, height)| DisplayMode { width, height })
                .collect(),
        })
    }

    pub fn apply(&mut self, width: u32, height: u32) -> Result<State, DisplayError> {
        if width == 0 && height == 0 {
            return self.restore();
        }
        let current = self.unchanged()?;
        if !self.backend.modes()?.iter().any(|mode| {
            mode.width == width && mode.height == height && compatible(mode, &current.mode)
        }) {
            return Err(DisplayError::UnsupportedMode);
        }
        let mut requested = current.mode;
        requested.width = width;
        requested.height = height;
        self.change(requested)
    }

    pub fn restore(&mut self) -> Result<State, DisplayError> {
        if let Err(error) = self.unchanged() {
            if error == DisplayError::ExternalChanged && self.journal_active {
                self.journal.remove(&self.owner)?;
                self.journal_active = false;
            }
            return Err(error);
        }
        let result = self.change(self.original.mode.clone())?;
        if self.journal_active {
            self.journal.remove(&self.owner)?;
            self.journal_active = false;
        }
        Ok(result)
    }

    fn unchanged(&mut self) -> Result<State, DisplayError> {
        let current = self.backend.current()?;
        if current != self.last {
            return Err(DisplayError::ExternalChanged);
        }
        Ok(current)
    }

    fn change(&mut self, requested: Mode) -> Result<State, DisplayError> {
        if requested == self.last.mode {
            return Ok(self.last.clone());
        }
        self.backend.test(&requested)?;
        self.unchanged()?;
        let previous = self.last.clone();
        self.journal.write(&Journal {
            version: 1,
            owner: self.owner.clone(),
            original: self.original.clone(),
            previous: previous.mode.clone(),
            requested: requested.clone(),
        })?;
        self.journal_active = true;
        // Retain the possibly applied state even if the subsequent OS query fails.
        self.last.mode = requested;
        let applied = self.backend.apply(&self.last.mode);
        let actual = self.backend.current()?;
        if applied.is_ok() && actual == self.last {
            return Ok(actual);
        }
        if actual != previous && actual != self.last {
            return Err(DisplayError::ExternalChanged);
        }
        if actual == self.last {
            self.backend.test(&previous.mode)?;
            self.unchanged()?;
            self.backend
                .apply(&previous.mode)
                .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
            self.last = previous.clone();
            if self.backend.current()? != previous {
                return Err(DisplayError::RecoveryFailed(
                    "rollback was not confirmed".into(),
                ));
            }
        }
        self.last = previous;
        Err(applied
            .err()
            .unwrap_or_else(|| DisplayError::Failed("requested mode was not applied".into())))
    }
}

impl<B: Backend, J: JournalStore> Drop for ModeSession<B, J> {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            self.journal.report_restore_error(&error);
        }
    }
}

fn compatible(candidate: &Mode, current: &Mode) -> bool {
    candidate.width > 0
        && candidate.height > 0
        && candidate.width <= 4096
        && candidate.height <= 4096
        && u64::from(candidate.width) * u64::from(candidate.height) <= 8_294_400
        && candidate.frequency == current.frequency
        && candidate.bits == current.bits
        && candidate.orientation == current.orientation
        && candidate.display_flags == current.display_flags
}

fn recover<B: Backend, J: JournalStore>(
    backend: &mut B,
    journal: &mut J,
    pending: &Journal,
) -> Result<(), DisplayError> {
    if pending.version != 1 {
        return Err(DisplayError::RecoveryFailed(
            "unknown recovery journal version".into(),
        ));
    }
    let current = backend.current()?;
    if current.device != pending.original.device || current.topology != pending.original.topology {
        return Err(DisplayError::ExternalChanged);
    }
    if current.mode == pending.original.mode {
        return journal.remove(&pending.owner);
    }
    if current.mode != pending.previous && current.mode != pending.requested {
        return Err(DisplayError::ExternalChanged);
    }
    backend
        .test(&pending.original.mode)
        .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
    if backend.current()? != current {
        return Err(DisplayError::ExternalChanged);
    }
    backend
        .apply(&pending.original.mode)
        .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
    if backend.current()? != pending.original {
        return Err(DisplayError::RecoveryFailed(
            "recovered mode was not confirmed".into(),
        ));
    }
    journal.remove(&pending.owner)
}
