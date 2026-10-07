use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{c_char, c_void, CStr, CString},
};

use serde::Deserialize;
use serde_json::{json, Value};

const MAX_EVENT_BYTES: usize = 8192;
const MAX_POINTS: usize = 16;
const MAX_KEYS: usize = 128;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    id: i64,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    Touch {
        action: String,
        points: Vec<Point>,
        #[serde(rename = "changedPoints")]
        changed_points: Vec<Point>,
        time: u64,
        width: f64,
        height: f64,
    },
    Mouse {
        action: String,
        button: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Wheel {
        action: String,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        discrete: bool,
        width: f64,
        height: f64,
    },
    Key {
        action: String,
        #[serde(rename = "physicalCode")]
        physical_code: u32,
        code: String,
    },
    Text {
        text: String,
    },
    Release {},
}

fn bounded(value: f64) -> bool {
    value.is_finite() && value.abs() <= 1_000_000.0
}

impl Event {
    fn valid(&self) -> bool {
        let size = |width: f64, height: f64| {
            bounded(width) && bounded(height) && width >= 0.0 && height >= 0.0
        };
        match self {
            Self::Touch {
                action,
                points,
                changed_points,
                width,
                height,
                ..
            } => {
                matches!(action.as_str(), "down" | "up" | "move" | "cancel")
                    && points.len() <= MAX_POINTS
                    && changed_points.len() <= MAX_POINTS
                    && points
                        .iter()
                        .chain(changed_points)
                        .all(|p| bounded(p.x) && bounded(p.y))
                    && size(*width, *height)
            }
            Self::Mouse {
                action,
                button,
                x,
                y,
                width,
                height,
            } => {
                matches!(action.as_str(), "press" | "release" | "move" | "cancel")
                    && matches!(button.as_str(), "left" | "right" | "middle" | "")
                    && bounded(*x)
                    && bounded(*y)
                    && size(*width, *height)
            }
            Self::Wheel {
                action,
                x,
                y,
                dx,
                dy,
                discrete,
                width,
                height,
            } => {
                matches!(action.as_str(), "begin" | "update" | "")
                    && bounded(*x)
                    && bounded(*y)
                    && bounded(*dx)
                    && bounded(*dy)
                    && (!discrete
                        || (dx.fract() == 0.0
                            && dy.fract() == 0.0
                            && dx.abs() <= 10.0
                            && dy.abs() <= 10.0))
                    && size(*width, *height)
            }
            Self::Key { action, code, .. } => {
                matches!(action.as_str(), "down" | "up" | "") && code.len() <= 64
            }
            Self::Text { text } => !text.contains('\0'),
            Self::Release {} => true,
        }
    }
}

#[derive(Default)]
struct TouchState {
    held: bool,
    id: Option<i64>,
    x: f64,
    y: f64,
    distance: f64,
    started: u64,
    scrolling: bool,
}

#[derive(Default)]
pub struct ControllerInput {
    touch: TouchState,
    buttons: BTreeSet<String>,
    keys: BTreeMap<u32, String>,
    scroll_x: f64,
    scroll_y: f64,
}

pub type ControllerInputSink = unsafe extern "C" fn(*mut c_void, *const c_char) -> i32;

struct Sink {
    callback: ControllerInputSink,
    context: *mut c_void,
}

impl Sink {
    fn emit(&self, command: Value) -> Result<(), i32> {
        let command = CString::new(command.to_string()).map_err(|_| 1)?;
        // The caller keeps the context alive and cannot reenter this input handle.
        match unsafe { (self.callback)(self.context, command.as_ptr()) } {
            0 => Ok(()),
            status => Err(status),
        }
    }

    fn button(&self, button: &str, down: bool) -> Result<(), i32> {
        self.emit(json!({"kind":"button","button":button,"down":down}))
    }

    fn position(&self, x: f64, y: f64, width: f64, height: f64, clamp: bool) -> Result<bool, i32> {
        if width <= 0.0 || height <= 0.0 {
            return Ok(false);
        }
        let (x, y) = if clamp {
            (x.clamp(0.0, width), y.clamp(0.0, height))
        } else {
            (x, y)
        };
        if x < 0.0 || y < 0.0 || x > width || y > height {
            return Ok(false);
        }
        self.emit(
            json!({"kind":"move","x":(x / width * 65535.0).round() as u32,
            "y":(y / height * 65535.0).round() as u32}),
        )?;
        Ok(true)
    }
}

impl ControllerInput {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn release(&mut self, sink: &Sink) -> Result<(), i32> {
        let held = self.touch.held || !self.buttons.is_empty() || !self.keys.is_empty();
        if held {
            sink.emit(json!({"kind":"release_all"}))?;
        }
        self.reset();
        Ok(())
    }

    fn scroll(&mut self, dx: f64, dy: f64, step: f64, sink: &Sink) -> Result<(), i32> {
        self.scroll_x += dx;
        self.scroll_y += dy;
        let x = (self.scroll_x / step).trunc().clamp(-10.0, 10.0) as i32;
        let y = (self.scroll_y / step).trunc().clamp(-10.0, 10.0) as i32;
        if x != 0 || y != 0 {
            sink.emit(json!({"kind":"wheel","dx":x,"dy":y}))?;
            self.scroll_x -= f64::from(x) * step;
            self.scroll_y -= f64::from(y) * step;
        }
        Ok(())
    }

    fn event(&mut self, event: Event, sink: &Sink) -> Result<(), i32> {
        match event {
            Event::Release {} => self.release(sink),
            Event::Text { text } => {
                let mut rest = text.as_str();
                while !rest.is_empty() {
                    let mut end = rest.len().min(512);
                    while !rest.is_char_boundary(end) {
                        end -= 1;
                    }
                    sink.emit(json!({"kind":"text","text":&rest[..end]}))?;
                    rest = &rest[end..];
                }
                Ok(())
            }
            Event::Key {
                action,
                physical_code,
                code,
            } => {
                let command = json!({"kind":"key","code":code,"down":action == "down"});
                if crate::input::parse_command(&command.to_string()).is_err() {
                    return Ok(());
                }
                if action == "down" {
                    if self.keys.len() >= MAX_KEYS && !self.keys.contains_key(&physical_code) {
                        return Err(1);
                    }
                    sink.emit(command)?;
                    self.keys.insert(physical_code, code);
                } else if action == "up" {
                    if let Some(held) = self.keys.get(&physical_code) {
                        if !self
                            .keys
                            .iter()
                            .any(|(id, code)| *id != physical_code && code == held)
                        {
                            sink.emit(json!({"kind":"key","code":held,"down":false}))?;
                        }
                        self.keys.remove(&physical_code);
                    }
                }
                Ok(())
            }
            Event::Mouse {
                action,
                button,
                x,
                y,
                width,
                height,
            } => {
                if action == "cancel" {
                    return self.release(sink);
                }
                let positioned = sink.position(x, y, width, height, !self.buttons.is_empty())?;
                if action == "press" && !button.is_empty() && positioned {
                    sink.button(&button, true)?;
                    self.buttons.insert(button);
                } else if action == "release" && self.buttons.contains(&button) {
                    sink.button(&button, false)?;
                    self.buttons.remove(&button);
                }
                Ok(())
            }
            Event::Wheel {
                action,
                x,
                y,
                dx,
                dy,
                discrete,
                width,
                height,
            } => {
                if !sink.position(x, y, width, height, false)? || action.is_empty() {
                    return Ok(());
                }
                if discrete {
                    sink.emit(json!({"kind":"wheel","dx":dx as i32,"dy":dy as i32}))
                } else {
                    self.scroll(dx, dy, 40.0, sink)
                }
            }
            Event::Touch {
                action,
                points,
                changed_points,
                time,
                width,
                height,
            } => {
                if action == "cancel" {
                    return self.release(sink);
                }
                if changed_points.is_empty() {
                    return Ok(());
                }
                if action == "down" && points.len() > 1 {
                    self.release(sink)?;
                    self.touch.scrolling = true;
                    self.touch.x = (points[0].x + points[1].x) / 2.0;
                    self.touch.y = (points[0].y + points[1].y) / 2.0;
                    return Ok(());
                }
                if self.touch.scrolling {
                    if action == "move" && points.len() == 2 {
                        let x = (points[0].x + points[1].x) / 2.0;
                        let y = (points[0].y + points[1].y) / 2.0;
                        let (dx, dy) = (self.touch.x - x, y - self.touch.y);
                        self.touch.x = x;
                        self.touch.y = y;
                        self.scroll(dx, dy, 24.0, sink)?;
                    } else if points.is_empty() {
                        self.release(sink)?;
                    }
                    return Ok(());
                }
                if action == "down" {
                    let point = &changed_points[0];
                    if sink.position(point.x, point.y, width, height, false)? {
                        self.touch.id = Some(point.id);
                        self.touch.x = point.x;
                        self.touch.y = point.y;
                        self.touch.distance = 0.0;
                        self.touch.started = time;
                    }
                    return Ok(());
                }
                for point in changed_points {
                    if self.touch.id != Some(point.id) {
                        continue;
                    }
                    self.touch.distance +=
                        (point.x - self.touch.x).abs() + (point.y - self.touch.y).abs();
                    if action == "move" {
                        self.touch.x = point.x;
                        self.touch.y = point.y;
                        if self.touch.distance >= 6.0 {
                            if !self.touch.held {
                                sink.button("left", true)?;
                                self.touch.held = true;
                            }
                            sink.position(point.x, point.y, width, height, true)?;
                        }
                    } else if action == "up" {
                        self.touch.id = None;
                        if self.touch.held {
                            sink.position(point.x, point.y, width, height, true)?;
                            sink.button("left", false)?;
                            self.touch.held = false;
                        } else if self.touch.distance < 6.0
                            && sink.position(point.x, point.y, width, height, false)?
                        {
                            let button = if time.saturating_sub(self.touch.started) >= 500 {
                                "right"
                            } else {
                                "left"
                            };
                            sink.button(button, true)?;
                            sink.button(button, false)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_input_new_v1() -> *mut ControllerInput {
    Box::into_raw(Box::new(ControllerInput::default()))
}

#[no_mangle]
pub extern "C" fn controller_input_event_v1(
    input: *mut ControllerInput,
    event_json: *const c_char,
    sink: Option<ControllerInputSink>,
    context: *mut c_void,
) -> i32 {
    if input.is_null() || event_json.is_null() {
        return 1;
    }
    let Some(callback) = sink else {
        return 1;
    };
    // The caller supplies a live, exclusive handle and a NUL-terminated event.
    let raw = unsafe { CStr::from_ptr(event_json) }.to_bytes();
    if raw.len() > MAX_EVENT_BYTES {
        return 1;
    }
    let Ok(event) = serde_json::from_slice::<Event>(raw) else {
        return 1;
    };
    if !event.valid() {
        return 1;
    }
    let input = unsafe { &mut *input };
    match input.event(event, &Sink { callback, context }) {
        Ok(()) => 0,
        Err(status) => {
            input.reset();
            status
        }
    }
}

#[no_mangle]
pub extern "C" fn controller_input_reset_v1(input: *mut ControllerInput) {
    if let Some(input) = unsafe { input.as_mut() } {
        input.reset();
    }
}

#[no_mangle]
pub extern "C" fn controller_input_free_v1(input: *mut ControllerInput) {
    if !input.is_null() {
        // Each handle is freed once, after all event calls return.
        unsafe { drop(Box::from_raw(input)) }
    }
}
