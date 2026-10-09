#[path = "secure_display_policy.rs"]
mod policy;

pub use policy::{DisplayError, DisplayMode, DisplaySnapshot};

use hbb_common::config::Config;
use policy::{Backend, Journal, JournalStore, Mode, ModeSession, State};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use winapi::um::{
    winbase::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
    wingdi::{
        DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP,
        DISPLAY_DEVICE_PRIMARY_DEVICE, DM_BITSPERPEL, DM_DISPLAYFIXEDOUTPUT, DM_DISPLAYFLAGS,
        DM_DISPLAYFREQUENCY, DM_DISPLAYORIENTATION, DM_PELSHEIGHT, DM_PELSWIDTH, DM_POSITION,
    },
    winuser::{
        ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsExW, CDS_TEST,
        DISP_CHANGE_SUCCESSFUL, ENUM_CURRENT_SETTINGS,
    },
};

static DISPLAY_OWNER: OnceLock<Arc<AtomicBool>> = OnceLock::new();

pub fn recover_pending() -> Result<(), DisplayError> {
    if FileJournal(Config::file().with_extension("display-recovery.json"))
        .read()?
        .is_some()
    {
        drop(DisplaySession::new()?);
    }
    Ok(())
}

pub struct DisplaySession(ModeSession<WindowsDisplay, FileJournal>);

impl DisplaySession {
    pub fn new() -> Result<Self, DisplayError> {
        let owner = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| DisplayError::Failed(error.to_string()))?
                .as_nanos()
        );
        let exclusive = DISPLAY_OWNER
            .get_or_init(|| Arc::new(AtomicBool::new(false)))
            .clone();
        Ok(Self(ModeSession::open(
            WindowsDisplay::new()?,
            FileJournal(Config::file().with_extension("display-recovery.json")),
            exclusive,
            owner,
        )?))
    }

    pub fn snapshot(&mut self) -> Result<DisplaySnapshot, DisplayError> {
        self.0.snapshot()
    }

    pub fn apply(&mut self, width: u32, height: u32) -> Result<DisplaySnapshot, DisplayError> {
        self.0.apply(width, height)?;
        self.snapshot()
    }

    pub fn restore(&mut self) -> Result<DisplaySnapshot, DisplayError> {
        self.0.restore()?;
        self.snapshot()
    }
}

struct WindowsDisplay {
    device: String,
    // DEVMODEW contains only owned scalar/array fields; the session is Send without
    // retaining thread-affine GDI handles or borrowing a driver-owned buffer.
    original: Option<(Mode, DEVMODEW)>,
}

impl WindowsDisplay {
    fn new() -> Result<Self, DisplayError> {
        let (device, _) = primary_topology()?;
        Ok(Self {
            device,
            original: None,
        })
    }

    fn native_mode(&self, mode: &Mode) -> Result<DEVMODEW, DisplayError> {
        if let Some((saved, native)) = &self.original {
            if saved == mode {
                return Ok(*native);
            }
        }
        let mut native = read_mode(&self.device, ENUM_CURRENT_SETTINGS)?;
        native.dmPelsWidth = mode.width;
        native.dmPelsHeight = mode.height;
        native.dmDisplayFrequency = mode.frequency;
        native.dmBitsPerPel = mode.bits;
        unsafe {
            let display = native.u1.s2_mut();
            display.dmPosition.x = mode.x;
            display.dmPosition.y = mode.y;
            display.dmDisplayOrientation = mode.orientation;
            display.dmDisplayFixedOutput = mode.fixed_output;
            *native.u2.dmDisplayFlags_mut() = mode.display_flags;
        }
        native.dmFields |= DM_PELSWIDTH
            | DM_PELSHEIGHT
            | DM_DISPLAYFREQUENCY
            | DM_BITSPERPEL
            | DM_DISPLAYORIENTATION
            | DM_DISPLAYFIXEDOUTPUT
            | DM_DISPLAYFLAGS
            | DM_POSITION;
        Ok(native)
    }

    fn change(&self, mode: &Mode, flags: u32) -> Result<(), DisplayError> {
        let mut native = self.native_mode(mode)?;
        let device = wide(&self.device);
        let result = unsafe {
            ChangeDisplaySettingsExW(
                device.as_ptr(),
                &mut native,
                std::ptr::null_mut(),
                flags,
                std::ptr::null_mut(),
            )
        };
        if result != DISP_CHANGE_SUCCESSFUL {
            return Err(DisplayError::Failed(format!(
                "display driver returned {result}"
            )));
        }
        Ok(())
    }
}

impl Backend for WindowsDisplay {
    fn current(&mut self) -> Result<State, DisplayError> {
        let (device, topology) = primary_topology()?;
        let mode = mode_fields(&read_mode(&device, ENUM_CURRENT_SETTINGS)?);
        Ok(State {
            device,
            topology,
            mode,
        })
    }

    fn capture_original(&mut self) -> Result<State, DisplayError> {
        let state = self.current()?;
        if state.device != self.device {
            return Err(DisplayError::ExternalChanged);
        }
        let native = read_mode(&self.device, ENUM_CURRENT_SETTINGS)?;
        if mode_fields(&native) != state.mode {
            return Err(DisplayError::ExternalChanged);
        }
        self.original = Some((state.mode.clone(), native));
        Ok(state)
    }

    fn modes(&mut self) -> Result<Vec<Mode>, DisplayError> {
        let mut modes = Vec::new();
        let device = wide(&self.device);
        for index in 0..4096 {
            let mut native = empty_mode();
            if unsafe { EnumDisplaySettingsExW(device.as_ptr(), index, &mut native, 0) } == 0 {
                break;
            }
            modes.push(mode_fields(&native));
        }
        Ok(modes)
    }

    fn test(&mut self, mode: &Mode) -> Result<(), DisplayError> {
        self.change(mode, CDS_TEST)
    }

    fn apply(&mut self, mode: &Mode) -> Result<(), DisplayError> {
        // Zero flags applies only to the running desktop and never persists a mode.
        self.change(mode, 0)
    }
}

fn empty_mode() -> DEVMODEW {
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    mode.dmSize = std::mem::size_of::<DEVMODEW>() as _;
    mode
}

fn read_mode(device: &str, index: u32) -> Result<DEVMODEW, DisplayError> {
    let mut mode = empty_mode();
    if unsafe { EnumDisplaySettingsExW(wide(device).as_ptr(), index, &mut mode, 0) } == 0 {
        return Err(DisplayError::Failed("cannot query display mode".into()));
    }
    Ok(mode)
}

fn mode_fields(native: &DEVMODEW) -> Mode {
    let display = unsafe { native.u1.s2() };
    Mode {
        width: native.dmPelsWidth,
        height: native.dmPelsHeight,
        frequency: native.dmDisplayFrequency,
        bits: native.dmBitsPerPel,
        orientation: display.dmDisplayOrientation,
        fixed_output: display.dmDisplayFixedOutput,
        display_flags: unsafe { *native.u2.dmDisplayFlags() },
        x: display.dmPosition.x,
        y: display.dmPosition.y,
    }
}

fn primary_topology() -> Result<(String, Vec<String>), DisplayError> {
    let mut primary = None;
    let mut topology = Vec::new();
    for index in 0..256 {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as _;
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) } == 0 {
            break;
        }
        if device.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
            continue;
        }
        let name = from_wide(&device.DeviceName);
        let is_primary = device.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0;
        if is_primary {
            primary = Some(name.clone());
        }
        topology.push(format!(
            "{name}|{}|{}",
            from_wide(&device.DeviceID),
            device.StateFlags
        ));
        // Include monitor identity so replacing a monitor on the same adapter is
        // not mistaken for the display on which this session obtained approval.
        for monitor_index in 0..256 {
            let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
            monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as _;
            if unsafe {
                EnumDisplayDevicesW(device.DeviceName.as_ptr(), monitor_index, &mut monitor, 0)
            } == 0
            {
                break;
            }
            topology.push(format!(
                "{name}|{}|{}|{}",
                from_wide(&monitor.DeviceName),
                from_wide(&monitor.DeviceID),
                monitor.StateFlags
            ));
        }
    }
    topology.sort_unstable();
    primary
        .map(|device| (device, topology))
        .ok_or_else(|| DisplayError::Failed("no active primary display".into()))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn from_wide(value: &[u16]) -> String {
    String::from_utf16_lossy(
        &value[..value
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(value.len())],
    )
}

struct FileJournal(PathBuf);

impl JournalStore for FileJournal {
    fn read(&mut self) -> Result<Option<Journal>, DisplayError> {
        match fs::metadata(&self.0) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(DisplayError::RecoveryFailed(error.to_string())),
            Ok(metadata) if metadata.len() > 65536 => {
                return Err(DisplayError::RecoveryFailed(
                    "invalid display journal size".into(),
                ))
            }
            Ok(_) => {}
        }
        let bytes =
            fs::read(&self.0).map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))
    }

    fn write(&mut self, journal: &Journal) -> Result<(), DisplayError> {
        if self
            .read()?
            .is_some_and(|current| current.owner != journal.owner)
        {
            return Err(DisplayError::Busy);
        }
        let bytes = serde_json::to_vec(journal)
            .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
        if let Some(parent) = self.0.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
        }
        let temporary = self.0.with_extension(format!("{}.tmp", journal.owner));
        let write = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            let source: Vec<_> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let destination: Vec<_> = self.0.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe {
                MoveFileExW(
                    source.as_ptr(),
                    destination.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        })();
        if let Err(error) = write {
            if let Err(cleanup) = fs::remove_file(&temporary) {
                if cleanup.kind() != std::io::ErrorKind::NotFound {
                    hbb_common::throttled_log!(
                        Duration::from_secs(5),
                        warn,
                        "secure display journal temporary cleanup failed: {cleanup}"
                    );
                }
            }
            return Err(DisplayError::RecoveryFailed(error.to_string()));
        }
        Ok(())
    }

    fn remove(&mut self, owner: &str) -> Result<(), DisplayError> {
        if let Some(journal) = self.read()? {
            if journal.owner != owner {
                return Err(DisplayError::RecoveryFailed(
                    "display journal belongs to another owner".into(),
                ));
            }
            fs::remove_file(&self.0)
                .map_err(|error| DisplayError::RecoveryFailed(error.to_string()))?;
        }
        Ok(())
    }

    fn report_restore_error(&self, error: &DisplayError) {
        hbb_common::throttled_log!(
            Duration::from_secs(5),
            warn,
            "secure display restore did not complete: {error}"
        );
    }
}
