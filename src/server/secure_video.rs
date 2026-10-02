use std::{
    mem::size_of,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use base::message_proto::Message;
use hbb_common::{bail, tokio::sync::mpsc, ResultType};
use scrap::{
    codec::{Encoder, EncoderCfg},
    vpxcodec::{VpxEncoderConfig, VpxVideoCodecId},
    ARGBToI420, EncodeInput, Pixfmt,
};
use winapi::{
    shared::{
        minwindef::FALSE,
        windef::{HBITMAP, HDC, HGDIOBJ},
    },
    um::{
        wingdi::{
            CreateCompatibleDC, CreateDCW, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush,
            GetDeviceCaps, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HALFTONE, HORZRES, SRCCOPY, VERTRES,
        },
        winuser::{
            CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_READOBJECTS,
            UOI_NAME,
        },
    },
};

const MAX_WIDTH: u32 = 1280;
const MAX_HEIGHT: u32 = 720;
const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const FRAME_PERIOD: Duration = Duration::from_millis(125);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

pub fn allowed() -> bool {
    gate_value(std::env::var("ORD_SECURE_VIDEO").ok().as_deref())
}

fn gate_value(value: Option<&str>) -> bool {
    value == Some("1")
}

fn scaled_dimensions(width: u32, height: u32) -> ResultType<Dimensions> {
    if width < 2 || height < 2 {
        bail!("Primary display is unavailable");
    }
    let (w, h) = if width <= MAX_WIDTH && height <= MAX_HEIGHT {
        (width, height)
    } else if (width as u64) * MAX_HEIGHT as u64 > (height as u64) * MAX_WIDTH as u64 {
        (
            MAX_WIDTH,
            ((height as u64 * MAX_WIDTH as u64) / width as u64) as u32,
        )
    } else {
        (
            ((width as u64 * MAX_HEIGHT as u64) / height as u64) as u32,
            MAX_HEIGHT,
        )
    };
    let result = Dimensions {
        width: w & !1,
        height: h & !1,
    };
    if result.width == 0 || result.height == 0 {
        bail!("Primary display is too narrow to encode");
    }
    Ok(result)
}

fn visible_desktop() -> ResultType<()> {
    if crate::platform::is_locked() {
        bail!("Interactive desktop is unavailable");
    }
    unsafe {
        let desktop = OpenInputDesktop(0, FALSE, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            bail!("Input desktop is unavailable");
        }
        let mut name = [0u16; 64];
        let mut needed = 0;
        let ok = GetUserObjectInformationW(
            desktop as _,
            UOI_NAME as _,
            name.as_mut_ptr() as _,
            (name.len() * size_of::<u16>()) as _,
            &mut needed,
        );
        CloseDesktop(desktop);
        if ok == 0 || needed == 0 || needed as usize > name.len() * size_of::<u16>() {
            bail!("Input desktop name is unavailable");
        }
        let end = name
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(name.len());
        if String::from_utf16_lossy(&name[..end]) != "Default" {
            bail!("Secure desktop cannot be captured");
        }
    }
    Ok(())
}

fn check_initial_desktop() -> ResultType<()> {
    if crate::platform::is_prelogin() || crate::platform::windows::is_logon_ui()? {
        bail!("Interactive desktop is unavailable");
    }
    visible_desktop()
}

fn primary_dc() -> ResultType<(HDC, u32, u32)> {
    let name: Vec<u16> = "DISPLAY\0".encode_utf16().collect();
    unsafe {
        let dc = CreateDCW(name.as_ptr(), ptr::null(), ptr::null(), ptr::null());
        if dc.is_null() {
            bail!("Primary display is unavailable");
        }
        let width = GetDeviceCaps(dc, HORZRES);
        let height = GetDeviceCaps(dc, VERTRES);
        if width < 2 || height < 2 {
            DeleteDC(dc);
            bail!("Primary display dimensions are unavailable");
        }
        Ok((dc, width as u32, height as u32))
    }
}

// Called only after the connection's CM Authorize has been consumed.
pub fn dimensions() -> ResultType<Dimensions> {
    check_initial_desktop()?;
    let (dc, width, height) = primary_dc()?;
    unsafe {
        DeleteDC(dc);
    }
    scaled_dimensions(width, height)
}

struct Capture {
    source: HDC,
    target: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    pixels: *mut u8,
    source_width: i32,
    source_height: i32,
    dimensions: Dimensions,
}

impl Capture {
    fn new(dimensions: Dimensions) -> ResultType<Self> {
        check_initial_desktop()?;
        let (source, width, height) = primary_dc()?;
        let measured = scaled_dimensions(width, height);
        if measured.as_ref().ok() != Some(&dimensions) {
            unsafe {
                DeleteDC(source);
            }
            bail!("Primary display changed");
        }
        unsafe {
            let target = CreateCompatibleDC(source);
            if target.is_null() {
                DeleteDC(source);
                bail!("Could not create capture DC");
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as _,
                biWidth: dimensions.width as _,
                biHeight: -(dimensions.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut pixels = ptr::null_mut();
            let bitmap = CreateDIBSection(
                source,
                &info,
                DIB_RGB_COLORS,
                &mut pixels,
                ptr::null_mut(),
                0,
            );
            if bitmap.is_null() || pixels.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap as _);
                }
                DeleteDC(target);
                DeleteDC(source);
                bail!("Could not allocate capture bitmap");
            }
            let old = SelectObject(target, bitmap as _);
            if old.is_null() || old as isize == -1 {
                DeleteObject(bitmap as _);
                DeleteDC(target);
                DeleteDC(source);
                bail!("Could not select capture bitmap");
            }
            SetStretchBltMode(target, HALFTONE);
            Ok(Self {
                source,
                target,
                bitmap,
                old,
                pixels: pixels as _,
                source_width: width as _,
                source_height: height as _,
                dimensions,
            })
        }
    }

    fn frame(&mut self) -> ResultType<&[u8]> {
        visible_desktop()?;
        unsafe {
            if GetDeviceCaps(self.source, HORZRES) != self.source_width
                || GetDeviceCaps(self.source, VERTRES) != self.source_height
                || StretchBlt(
                    self.target,
                    0,
                    0,
                    self.dimensions.width as _,
                    self.dimensions.height as _,
                    self.source,
                    0,
                    0,
                    self.source_width,
                    self.source_height,
                    SRCCOPY,
                ) == 0
            {
                bail!("Primary display capture failed");
            }
            if GdiFlush() == 0 {
                bail!("Primary display capture synchronization failed");
            }
            visible_desktop()?;
            let len = self.dimensions.width as usize * self.dimensions.height as usize * 4;
            Ok(std::slice::from_raw_parts(self.pixels, len))
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.target, self.old);
            DeleteObject(self.bitmap as _);
            DeleteDC(self.target);
            DeleteDC(self.source);
        }
    }
}

fn run_worker(
    dimensions: Dimensions,
    tx: mpsc::Sender<Result<Message, ()>>,
    cancelled: Arc<AtomicBool>,
) -> ResultType<()> {
    let mut capture = Capture::new(dimensions)?;
    let mut encoder = Encoder::new(
        EncoderCfg::VPX(VpxEncoderConfig {
            width: dimensions.width,
            height: dimensions.height,
            quality: 1.0,
            codec: VpxVideoCodecId::VP8,
            keyframe_interval: Some(32),
        }),
        false,
    )?;
    let format = encoder.yuvfmt();
    if format.pixfmt != Pixfmt::I420 || format.stride.len() < 3 {
        bail!("VP8 encoder rejected I420 layout");
    }
    let yuv_len = format.v + format.stride[2] * (format.h / 2);
    let mut yuv = vec![0u8; yuv_len];
    let mut previous = Vec::new();
    let started = Instant::now();
    while !cancelled.load(Ordering::Acquire) && !tx.is_closed() {
        let tick = Instant::now();
        let pixels = capture.frame()?;
        if previous.as_slice() != pixels {
            previous.clear();
            previous.extend_from_slice(pixels);
            let converted = unsafe {
                ARGBToI420(
                    pixels.as_ptr(),
                    (dimensions.width * 4) as _,
                    yuv.as_mut_ptr(),
                    format.stride[0] as _,
                    yuv[format.u..].as_mut_ptr(),
                    format.stride[1] as _,
                    yuv[format.v..].as_mut_ptr(),
                    format.stride[2] as _,
                    dimensions.width as _,
                    dimensions.height as _,
                )
            };
            if converted != 0 {
                bail!("Screen color conversion failed");
            }
            let frame = encoder
                .encode_to_message(EncodeInput::YUV(&yuv), started.elapsed().as_millis() as i64)?;
            let Some(base::message_proto::video_frame::Union::Vp8s(vp8s)) = &frame.union else {
                bail!("VP8 encoder returned another codec");
            };
            if vp8s.frames.is_empty()
                || vp8s.frames.len() > 4
                || vp8s
                    .frames
                    .iter()
                    .any(|f| f.data.is_empty() || f.data.len() > MAX_FRAME_BYTES)
            {
                bail!("VP8 frame exceeds secure video limits");
            }
            let mut message = Message::new();
            message.set_video_frame(frame);
            if hbb_common::protobuf::Message::compute_size(&message) as usize > MAX_MESSAGE_BYTES {
                bail!("VP8 message exceeds secure video limits");
            }
            if tx.blocking_send(Ok(message)).is_err() {
                return Ok(());
            }
        }
        let wait = FRAME_PERIOD.saturating_sub(tick.elapsed());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
    }
    Ok(())
}

pub struct Worker {
    pub receiver: mpsc::Receiver<Result<Message, ()>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.receiver.close();
    }
}

pub fn start(dimensions: Dimensions) -> ResultType<Worker> {
    if !allowed() {
        bail!("Secure video is disabled locally");
    }
    let (tx, receiver) = mpsc::channel(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    std::thread::Builder::new()
        .name("ord-secure-video".to_owned())
        .spawn(move || {
            if let Err(error) = run_worker(dimensions, tx.clone(), flag) {
                hbb_common::log::warn!("Secure video stopped: {error}");
                let _ = tx.blocking_send(Err(()));
            }
        })?;
    Ok(Worker {
        receiver,
        cancelled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_and_even_dimensions_preserve_aspect_ratio() {
        assert_eq!(
            scaled_dimensions(1920, 1080).unwrap(),
            Dimensions {
                width: 1280,
                height: 720
            }
        );
        assert_eq!(
            scaled_dimensions(1024, 768).unwrap(),
            Dimensions {
                width: 960,
                height: 720
            }
        );
        assert_eq!(
            scaled_dimensions(801, 601).unwrap(),
            Dimensions {
                width: 800,
                height: 600
            }
        );
        assert_eq!(
            scaled_dimensions(640, 240).unwrap(),
            Dimensions {
                width: 640,
                height: 240
            }
        );
        assert!(scaled_dimensions(0, 720).is_err());
    }

    #[test]
    fn video_gate_requires_exact_local_opt_in() {
        assert!(!gate_value(None));
        assert!(!gate_value(Some("true")));
        assert!(!gate_value(Some("0")));
        assert!(gate_value(Some("1")));
    }

    #[test]
    fn controlled_vp8_frame_decodes_to_expected_dimensions() {
        use scrap::vpxcodec::{VpxDecoder, VpxDecoderConfig};

        let dimensions = Dimensions {
            width: 64,
            height: 48,
        };
        let mut encoder = Encoder::new(
            EncoderCfg::VPX(VpxEncoderConfig {
                width: dimensions.width,
                height: dimensions.height,
                quality: 1.0,
                codec: VpxVideoCodecId::VP8,
                keyframe_interval: Some(32),
            }),
            false,
        )
        .unwrap();
        let format = encoder.yuvfmt();
        let mut yuv = vec![0u8; format.v + format.stride[2] * (format.h / 2)];
        yuv[..format.u].fill(96);
        yuv[format.u..format.v].fill(128);
        yuv[format.v..].fill(128);
        let encoded = encoder
            .encode_to_message(EncodeInput::YUV(&yuv), 0)
            .unwrap();
        let Some(base::message_proto::video_frame::Union::Vp8s(frames)) = encoded.union else {
            panic!("expected VP8");
        };
        assert!(frames.frames[0].key);
        if let Some(path) = std::env::var_os("ORD_TEST_VP8_OUT") {
            std::fs::write(std::path::PathBuf::from(path), &frames.frames[0].data).unwrap();
        }
        let mut decoder = VpxDecoder::new(VpxDecoderConfig {
            codec: VpxVideoCodecId::VP8,
        })
        .unwrap();
        let decoded = decoder
            .decode(&frames.frames[0].data)
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(decoded.inner().d_w, dimensions.width);
        assert_eq!(decoded.inner().d_h, dimensions.height);
    }

    #[test]
    fn dropping_worker_wakes_full_queue_producer() {
        let (tx, receiver) = mpsc::channel(1);
        tx.try_send(Ok(Message::new())).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            receiver,
            cancelled: Arc::clone(&cancelled),
        };
        let blocked = std::thread::spawn(move || tx.blocking_send(Ok(Message::new())));
        drop(worker);
        assert!(cancelled.load(Ordering::Acquire));
        assert!(blocked.join().unwrap().is_err());
    }
}
