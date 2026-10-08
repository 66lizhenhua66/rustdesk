use std::{
    ffi::{c_char, c_void, CStr, CString},
    ptr,
};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::interaction::{
    controller_input_event_v1, controller_input_free_v1, controller_input_new_v1,
    controller_input_reset_v1, ControllerInput,
    ControllerInputSink,
};

const MAX_EVENT_BYTES: usize = 8192;
const PAN_THRESHOLD: f64 = 6.0;
const SAFE_PADDING: f64 = 24.0;

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Insets {
    #[serde(default)]
    left: f64,
    #[serde(default)]
    top: f64,
    #[serde(default)]
    right: f64,
    #[serde(default)]
    bottom: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configure {
    #[serde(default)]
    #[serde(rename = "kind")]
    _kind: String,
    viewport_width: f64,
    viewport_height: f64,
    remote_width: f64,
    remote_height: f64,
    #[serde(default)]
    insets: Insets,
    #[serde(default = "default_padding")]
    padding: f64,
    #[serde(default)]
    full_viewport: bool,
    enabled: bool,
}

fn default_padding() -> f64 {
    SAFE_PADDING
}

#[derive(Clone, Copy, Default)]
struct Pt {
    id: i64,
    x: f64,
    y: f64,
}

#[derive(Default)]
struct Single {
    start: Pt,
    last: Pt,
    time: u64,
    moved: bool,
    ready: bool,
    dragging: bool,
    hit: bool,
}

#[derive(Default)]
struct Multi {
    start_centroid: Pt,
    last_centroid: Pt,
    start_distance: f64,
    last_distance: f64,
    started: u64,
    pinch: bool,
    scroll: bool,
    third: bool,
    remaining: Option<Pt>,
    hit: bool,
}

#[derive(Default)]
struct Gesture {
    single: Option<Single>,
    multi: Option<Multi>,
    suppress: bool,
    next_tick: u64,
}

pub struct ControllerCanvas {
    input: *mut ControllerInput,
    viewport_w: f64,
    viewport_h: f64,
    remote_w: f64,
    remote_h: f64,
    inset: Insets,
    padding: f64,
    full_viewport: bool,
    safe_x: f64,
    safe_y: f64,
    safe_w: f64,
    safe_h: f64,
    fit: f64,
    zoom: f64,
    image_x: f64,
    image_y: f64,
    valid: bool,
    enabled: bool,
    touch_mode: bool,
    cursor_x: f64,
    cursor_y: f64,
    remote_held: bool,
    gesture: Gesture,
}

impl ControllerCanvas {
    fn fields_only(v: &Value, allowed: &[&str]) -> bool {
        v.as_object()
            .map_or(false, |o| o.keys().all(|k| allowed.iter().any(|a| a == k)))
    }
    fn new() -> Self {
        Self {
            input: controller_input_new_v1(),
            viewport_w: 0.0,
            viewport_h: 0.0,
            remote_w: 0.0,
            remote_h: 0.0,
            inset: Insets::default(),
            padding: SAFE_PADDING,
            full_viewport: false,
            safe_x: 0.0,
            safe_y: 0.0,
            safe_w: 0.0,
            safe_h: 0.0,
            fit: 0.0,
            zoom: 1.0,
            image_x: 0.0,
            image_y: 0.0,
            valid: false,
            enabled: false,
            touch_mode: false,
            cursor_x: 0.0,
            cursor_y: 0.0,
            gesture: Gesture::default(),
            remote_held: false,
        }
    }

    fn scale(&self) -> f64 {
        self.fit * self.zoom
    }
    fn image_w(&self) -> f64 {
        self.remote_w * self.scale()
    }
    fn image_h(&self) -> f64 {
        self.remote_h * self.scale()
    }
    fn geometry_changed(&self, c: &Configure) -> bool {
        self.viewport_w != c.viewport_width
            || self.viewport_h != c.viewport_height
            || self.remote_w != c.remote_width
            || self.remote_h != c.remote_height
            || self.inset.left != c.insets.left
            || self.inset.top != c.insets.top
            || self.inset.right != c.insets.right
            || self.inset.bottom != c.insets.bottom
            || self.padding != c.padding
            || self.full_viewport != c.full_viewport
    }
    fn clamp_image(&mut self) {
        let iw = self.image_w();
        let ih = self.image_h();
        if !self.valid || iw <= 0.0 || ih <= 0.0 {
            self.image_x = self.safe_x;
            self.image_y = self.safe_y;
            return;
        }
        if self.full_viewport {
            self.clamp_full_viewport();
            return;
        }
        let min_x = self.safe_x + self.safe_w - iw;
        let max_x = self.safe_x;
        let min_y = self.safe_y + self.safe_h - ih;
        let max_y = self.safe_y;
        self.image_x = if iw <= self.safe_w {
            self.safe_x + (self.safe_w - iw) / 2.0
        } else {
            self.image_x.clamp(min_x, max_x)
        };
        self.image_y = if ih <= self.safe_h {
            self.safe_y + (self.safe_h - ih) / 2.0
        } else {
            self.image_y.clamp(min_y, max_y)
        };
    }
    fn clamp_full_viewport(&mut self) {
        let iw = self.image_w();
        let ih = self.image_h();
        let center_x = (self.viewport_w - iw) / 2.0;
        let center_y = (self.viewport_h - ih) / 2.0;
        if self.zoom <= 1.0 {
            self.image_x = center_x;
            self.image_y = center_y;
            return;
        }
        // Let remote edges clear the input-safe boundary while the image stays
        // centered at 1x, including when it is smaller than the viewport.
        self.image_x = self.image_x.clamp(
            (center_x - SAFE_PADDING).min(self.safe_x + self.safe_w - iw - SAFE_PADDING),
            (center_x + SAFE_PADDING).max(self.safe_x + SAFE_PADDING),
        );
        self.image_y = self.image_y.clamp(
            (center_y - SAFE_PADDING).min(self.safe_y + self.safe_h - ih - SAFE_PADDING),
            (center_y + SAFE_PADDING).max(self.safe_y + SAFE_PADDING),
        );
    }
    fn configure(
        &mut self,
        mut c: Configure,
        sink: Option<ControllerInputSink>,
        ctx: *mut c_void,
    ) -> i32 {
        if ![
            c.viewport_width,
            c.viewport_height,
            c.remote_width,
            c.remote_height,
            c.padding,
            c.insets.left,
            c.insets.top,
            c.insets.right,
            c.insets.bottom,
        ]
        .iter()
        .all(|v| v.is_finite())
            || c.viewport_width < 0.0
            || c.viewport_height < 0.0
            || c.remote_width <= 0.0
            || c.remote_height <= 0.0
            || c.padding < 0.0
            || c.insets.left < 0.0
            || c.insets.top < 0.0
            || c.insets.right < 0.0
            || c.insets.bottom < 0.0
        {
            let status = self.cancel(sink, ctx);
            if status != 0 {
                return status;
            }
            self.valid = false;
            self.viewport_w = c.viewport_width.max(0.0);
            self.viewport_h = c.viewport_height.max(0.0);
            self.remote_w = 0.0;
            self.remote_h = 0.0;
            self.safe_x = 0.0;
            self.safe_y = 0.0;
            self.safe_w = 0.0;
            self.safe_h = 0.0;
            self.fit = 0.0;
            self.zoom = 1.0;
            self.image_x = 0.0;
            self.image_y = 0.0;
            self.enabled = c.enabled;
            return 0;
        }
        if c.full_viewport
            && (c.viewport_width <= c.insets.left + c.insets.right + 2.0 * c.padding
                || c.viewport_height <= c.insets.top + c.insets.bottom + 2.0 * c.padding)
        {
            c.enabled = false;
        }
        let initialize_pointer = c.enabled && (!self.enabled || !self.valid);
        let changed = self.geometry_changed(&c);
        if !changed {
            if self.enabled && !c.enabled {
                let status = self.release(sink, ctx);
                if status != 0 {
                    return status;
                }
            }
            self.enabled = c.enabled;
            if initialize_pointer {
                return self.initialize_pointer(sink, ctx);
            }
            return 0;
        }
        let old_center = if changed && self.valid && self.scale() > 0.0 {
            let cx = self.safe_x + self.safe_w / 2.0;
            let cy = self.safe_y + self.safe_h / 2.0;
            let (cx, cy) = if self.full_viewport {
                (self.viewport_w / 2.0, self.viewport_h / 2.0)
            } else {
                (cx, cy)
            };
            Some((
                (cx - self.image_x) / self.image_w(),
                (cy - self.image_y) / self.image_h(),
            ))
        } else {
            None
        };
        let old_cursor = if self.remote_w > 0.0 && self.remote_h > 0.0 {
            Some((self.cursor_x / self.remote_w, self.cursor_y / self.remote_h))
        } else {
            None
        };
        if changed {
            let status = self.cancel(sink, ctx);
            if status != 0 {
                return status;
            }
        }
        self.viewport_w = c.viewport_width;
        self.viewport_h = c.viewport_height;
        self.remote_w = c.remote_width;
        self.remote_h = c.remote_height;
        self.inset = c.insets;
        self.padding = c.padding;
        self.full_viewport = c.full_viewport;
        self.safe_x = c.insets.left + c.padding;
        self.safe_y = c.insets.top + c.padding;
        self.safe_w =
            (c.viewport_width - c.insets.left - c.insets.right - 2.0 * c.padding).max(0.0);
        self.safe_h =
            (c.viewport_height - c.insets.top - c.insets.bottom - 2.0 * c.padding).max(0.0);
        self.fit = (self.safe_w / c.remote_width).min(self.safe_h / c.remote_height);
        if self.full_viewport {
            self.fit = (c.viewport_width / c.remote_width).min(c.viewport_height / c.remote_height);
        }
        self.valid = self.fit.is_finite() && self.fit > 0.0;
        if self.valid {
            if let Some((nx, ny)) = old_center {
                self.image_x = self.safe_x + self.safe_w / 2.0 - nx * self.image_w();
                self.image_y = self.safe_y + self.safe_h / 2.0 - ny * self.image_h();
            } else {
                self.image_x = self.safe_x + (self.safe_w - self.image_w()) / 2.0;
                self.image_y = self.safe_y + (self.safe_h - self.image_h()) / 2.0;
            }
            if self.full_viewport {
                let (nx, ny) = old_center.unwrap_or((0.5, 0.5));
                self.image_x = self.viewport_w / 2.0 - nx * self.image_w();
                self.image_y = self.viewport_h / 2.0 - ny * self.image_h();
            }
            self.clamp_image();
            let (cursor_x, cursor_y) = old_cursor.unwrap_or((0.5, 0.5));
            self.cursor_x = (cursor_x * c.remote_width).clamp(0.0, c.remote_width);
            self.cursor_y = (cursor_y * c.remote_height).clamp(0.0, c.remote_height);
        }
        if self.enabled && !c.enabled {
            let status = self.release(sink, ctx);
            if status != 0 {
                return status;
            }
        }
        self.enabled = c.enabled;
        if initialize_pointer {
            return self.initialize_pointer(sink, ctx);
        }
        0
    }
    fn cancel(&mut self, sink: Option<ControllerInputSink>, ctx: *mut c_void) -> i32 {
        self.gesture = Gesture::default();
        self.remote_held = false;
        if let Some(cb) = sink {
            self.release_with(cb, ctx)
        } else {
            0
        }
    }
    fn release(&mut self, sink: Option<ControllerInputSink>, ctx: *mut c_void) -> i32 {
        self.cancel(sink, ctx)
    }
    fn release_with(&mut self, cb: ControllerInputSink, ctx: *mut c_void) -> i32 {
        let status = controller_input_event_v1(self.input, c"{\"kind\":\"release\"}".as_ptr(), Some(cb), ctx);
        if status != 0 {
            self.fail_input();
        }
        status
    }
    fn fail_input(&mut self) {
        self.gesture = Gesture::default();
        self.remote_held = false;
        self.enabled = false;
        controller_input_reset_v1(self.input);
    }
    fn suspend(&mut self) {
        self.fail_input();
    }
    fn local_to_remote(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let s = self.scale();
        if !self.valid || s <= 0.0 || !self.in_safe(x, y) {
            return None;
        }
        let rx = (x - self.image_x) / s;
        let ry = (y - self.image_y) / s;
        if rx < 0.0 || ry < 0.0 || rx > self.remote_w || ry > self.remote_h {
            None
        } else {
            Some((rx, ry))
        }
    }
    fn in_safe(&self, x: f64, y: f64) -> bool {
        x >= self.safe_x && y >= self.safe_y
            && x <= self.safe_x + self.safe_w && y <= self.safe_y + self.safe_h
    }
    fn local_to_remote_clamped(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if !self.valid || self.scale() <= 0.0 {
            return None;
        }
        let x = x.clamp(self.safe_x, self.safe_x + self.safe_w);
        let y = y.clamp(self.safe_y, self.safe_y + self.safe_h);
        Some((((x - self.image_x) / self.scale()).clamp(0.0, self.remote_w),
              ((y - self.image_y) / self.scale()).clamp(0.0, self.remote_h)))
    }
    fn input_event(&mut self, value: Value, sink: ControllerInputSink, ctx: *mut c_void) -> i32 {
        if !self.enabled || !self.valid {
            return 0;
        }
        let Ok(raw) = CString::new(value.to_string()) else {
            return 1;
        };
        let status = controller_input_event_v1(self.input, raw.as_ptr(), Some(sink), ctx);
        if status != 0 {
            self.fail_input();
        }
        status
    }
    fn mouse(
        &mut self,
        action: &str,
        button: &str,
        x: f64,
        y: f64,
        sink: ControllerInputSink,
        ctx: *mut c_void,
    ) -> i32 {
        if action == "cancel" {
            return self.cancel(Some(sink), ctx);
        }
        let Some((rx, ry)) = self.local_to_remote(x, y) else {
            if action == "release" && self.remote_held {
                let Some((rx, ry)) = self.local_to_remote_clamped(x, y) else { return 0 };
                let status=self.input_event(json!({"kind":"mouse","action":"release","button":button,"x":rx,"y":ry,"width":self.remote_w,"height":self.remote_h}), sink, ctx);
                if status == 0 {
                    self.remote_held = false;
                }
                return status;
            }
            return 0;
        };
        let status=self.input_event(json!({"kind":"mouse","action":action,"button":button,"x":rx,"y":ry,"width":self.remote_w,"height":self.remote_h}), sink, ctx);
        if status == 0 && action == "press" && !button.is_empty() {
            self.remote_held = true;
        }
        if status == 0 && action == "release" {
            self.remote_held = false;
        }
        status
    }
    fn wheel(
        &mut self,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        discrete: bool,
        sink: ControllerInputSink,
        ctx: *mut c_void,
    ) -> i32 {
        let Some((rx, ry)) = self.local_to_remote(x, y) else {
            return 0;
        };
        self.input_event(json!({"kind":"wheel","action":"update","x":rx,"y":ry,"dx":dx,"dy":dy,"discrete":discrete,"width":self.remote_w,"height":self.remote_h}), sink, ctx)
    }
    fn pointer_mouse(&mut self, action: &str, button: &str, sink: ControllerInputSink, ctx: *mut c_void) -> i32 {
        self.input_event(json!({"kind":"mouse","action":action,"button":button,"x":self.cursor_x,"y":self.cursor_y,"width":self.remote_w,"height":self.remote_h}), sink, ctx)
    }
    fn initialize_pointer(&mut self, sink: Option<ControllerInputSink>, ctx: *mut c_void) -> i32 {
        if !self.touch_mode || !self.enabled || !self.valid || self.safe_w <= 0.0 || self.safe_h <= 0.0 {
            return 0;
        }
        let Some(sink) = sink else { return 0 };
        let Some((x, y)) = self.local_to_remote_clamped(self.viewport_w / 2.0, self.viewport_h / 2.0) else {
            return 0;
        };
        self.cursor_x = x;
        self.cursor_y = y;
        self.pointer_mouse("move", "", sink, ctx)
    }
    fn pointer_wheel(&mut self, dx: f64, dy: f64, sink: ControllerInputSink, ctx: *mut c_void) -> i32 {
        self.input_event(json!({"kind":"wheel","action":"update","x":self.cursor_x,"y":self.cursor_y,"dx":dx,"dy":dy,"discrete":true,"width":self.remote_w,"height":self.remote_h}), sink, ctx)
    }
    fn points(v: &Value, key: &str) -> Option<Vec<Pt>> {
        v.get(key)?
            .as_array()?
            .iter()
            .map(|p| {
                let object = p.as_object()?;
                if !object
                    .keys()
                    .all(|k| matches!(k.as_str(), "id" | "x" | "y"))
                {
                    return None;
                }
                let id = p.get("id")?.as_i64()?;
                let x = p.get("x")?.as_f64()?;
                let y = p.get("y")?.as_f64()?;
                if !x.is_finite()
                    || !y.is_finite()
                    || x.abs() > 1_000_000.0
                    || y.abs() > 1_000_000.0
                {
                    return None;
                }
                Some(Pt { id, x, y })
            })
            .collect()
    }
    fn centroid(points: &[Pt]) -> Pt {
        let n = points.len() as f64;
        Pt {
            id: 0,
            x: points.iter().map(|p| p.x).sum::<f64>() / n,
            y: points.iter().map(|p| p.y).sum::<f64>() / n,
        }
    }
    fn distance(points: &[Pt]) -> f64 {
        if points.len() < 2 {
            0.0
        } else {
            ((points[0].x - points[1].x).powi(2) + (points[0].y - points[1].y).powi(2)).sqrt()
        }
    }
    fn touch(&mut self, value: &Value, sink: ControllerInputSink, ctx: *mut c_void) -> i32 {
        let action = value.get("action").and_then(Value::as_str).unwrap_or("");
        let Some(points) = Self::points(value, "points") else {
            return 1;
        };
        let Some(changed) = Self::points(value, "changedPoints") else {
            return 1;
        };
        let Some(time) = value.get("time").and_then(Value::as_u64) else {
            return 1;
        };
        if !matches!(action, "down" | "up" | "move" | "cancel")
            || points.len() > 16
            || changed.len() > 16
        {
            return 1;
        }
        if action == "cancel" {
            return self.cancel(Some(sink), ctx);
        }
        if !self.valid {
            return 0;
        }
        if points.len() >= 3 {
            if self.gesture.multi.as_ref().map_or(false, |m| m.third) {
                return 0;
            }
            self.gesture.suppress = true;
            self.gesture.single = None;
            self.gesture.next_tick = 0;
            let status = self.cancel_input(sink, ctx);
            if status != 0 { return status; }
            self.gesture
                .multi
                .get_or_insert_with(Default::default)
                .third = true;
            return 0;
        }
        if self.gesture.multi.as_ref().map_or(false, |m| m.third) {
            if points.is_empty() {
                self.gesture = Gesture::default();
            }
            return 0;
        }
        if points.len() >= 2 {
            if self.gesture.multi.is_none() {
                self.gesture.single = None;
                self.gesture.suppress = true;
                self.gesture.next_tick = 0;
                let c = Self::centroid(&points);
                let d = Self::distance(&points);
                self.gesture.multi = Some(Multi {
                    start_centroid: c,
                    last_centroid: c,
                    start_distance: d,
                    last_distance: d,
                    started: time,
                    hit: points.iter().all(|p| self.in_safe(p.x, p.y)),
                    ..Default::default()
                });
                return self.cancel_input(sink, ctx);
            }
            let Some(mut m) = self.gesture.multi.take() else {
                return 0;
            };
            let c = Self::centroid(&points);
            let d = Self::distance(&points);
            let delta_c =
                ((c.x - m.start_centroid.x).powi(2) + (c.y - m.start_centroid.y).powi(2)).sqrt();
            let dist_change = (d - m.start_distance).abs();
            if !m.pinch
                && (dist_change >= 8.0
                    || (m.start_distance > 0.0 && dist_change >= m.start_distance * 0.05))
            {
                m.pinch = true;
            }
            if m.pinch {
                let factor = if m.last_distance > 0.0 {
                    d / m.last_distance
                } else {
                    1.0
                };
                let focal = c;
                let focal_remote = self.local_to_remote_clamped(focal.x, focal.y);
                self.zoom = (self.zoom * factor).clamp(1.0, 10.0);
                if let Some((rx, ry)) = focal_remote {
                    self.image_x = focal.x - rx * self.scale();
                    self.image_y = focal.y - ry * self.scale();
                    self.clamp_image();
                }
                m.last_distance = d;
                m.last_centroid = c;
                self.gesture.multi = Some(m);
                return 0;
            }
            if delta_c >= 8.0 || m.scroll {
                m.scroll = true;
                let dx = ((m.last_centroid.x - c.x) / 24.0).trunc() as i32;
                let dy = ((c.y - m.last_centroid.y) / 24.0).trunc() as i32;
                if dx != 0 || dy != 0 {
                    let status = if self.touch_mode && m.hit {
                        self.pointer_wheel(dx as f64, dy as f64, sink, ctx)
                    } else if self.touch_mode {
                        0
                    } else {
                        self.wheel(c.x, c.y, dx as f64, dy as f64, true, sink, ctx)
                    };
                    if status != 0 {
                        return status;
                    }
                    m.last_centroid = c;
                }
            }
            self.gesture.multi = Some(m);
            return 0;
        }
        if let Some(mut m) = self.gesture.multi.take() {
            if points.len() == 1 {
                let p = points[0];
                if let Some(previous) = m.remaining.filter(|previous| previous.id == p.id) {
                    let dx = p.x - previous.x;
                    let dy = p.y - previous.y;
                    if m.pinch {
                        self.image_x += dx;
                        self.image_y += dy;
                        self.clamp_image();
                    } else if dx.abs() + dy.abs() > PAN_THRESHOLD {
                        m.scroll = true;
                    }
                }
                m.remaining = Some(p);
                self.gesture.multi = Some(m);
                return 0;
            }
            if action == "up" && points.is_empty() {
                let tap = !m.pinch
                    && !m.scroll
                    && !m.third
                    && time.saturating_sub(m.started) <= 300
                    && ((m.last_centroid.x - m.start_centroid.x).powi(2)
                        + (m.last_centroid.y - m.start_centroid.y).powi(2))
                    .sqrt()
                        < 8.0
                    && (m.last_distance - m.start_distance).abs() < 8.0;
                let c = m.start_centroid;
                self.gesture.multi = None;
                self.gesture.suppress = false;
                if tap && (!self.touch_mode || m.hit) {
                    let status = if self.touch_mode {
                        self.pointer_mouse("press", "right", sink, ctx)
                    } else {
                        self.mouse("press", "right", c.x, c.y, sink, ctx)
                    };
                    if status != 0 { return status; }
                    return if self.touch_mode {
                        self.pointer_mouse("release", "right", sink, ctx)
                    } else {
                        self.mouse("release", "right", c.x, c.y, sink, ctx)
                    };
                }
                return 0;
            }
            self.gesture.multi = Some(m);
            return 0;
        }
        if self.touch_mode {
            return self.pointer_touch(action, &points, &changed, time, sink, ctx);
        }
        if action == "down" && points.len() == 1 {
            let p = points[0];
            self.gesture.single = Some(Single {
                start: p,
                last: p,
                time,
                hit: self.local_to_remote(p.x, p.y).is_some(),
                ..Default::default()
            });
            self.gesture.next_tick = if self.enabled && self.gesture.single.as_ref().map_or(false, |s| s.hit) {
                time.saturating_add(1000)
            } else { 0 };
            return 0;
        }
        if let Some(mut s) = self.gesture.single.take() {
            let p = changed
                .iter().find(|p| p.id == s.start.id)
                .copied()
                .or_else(|| points.iter().find(|p| p.id == s.start.id).copied())
                .unwrap_or(s.last);
            let dx = p.x - s.last.x;
            let dy = p.y - s.last.y;
            let total = ((p.x - s.start.x).powi(2) + (p.y - s.start.y).powi(2)).sqrt();
            if action == "move" {
                s.last = p;
                if !s.ready && total > PAN_THRESHOLD {
                    s.moved = true;
                    self.gesture.next_tick = 0;
                    if self.zoom > 1.0 {
                        self.image_x += dx;
                        self.image_y += dy;
                        self.clamp_image();
                    }
                } else if s.ready && !s.dragging && (dx != 0.0 || dy != 0.0) {
                    if s.hit && self.enabled {
                        let status = self.mouse("move", "", s.start.x, s.start.y, sink, ctx);
                        if status != 0 { return status; }
                        let status = self.mouse("press", "left", s.start.x, s.start.y, sink, ctx);
                        if status != 0 { return status; }
                        s.dragging = true;
                        if self.local_to_remote(p.x, p.y).is_none() {
                            return self.mouse("release", "left", p.x, p.y, sink, ctx);
                        }
                        let status = self.mouse("move", "", p.x, p.y, sink, ctx);
                        if status != 0 { return status; }
                    }
                } else if s.dragging {
                    if self.local_to_remote(p.x, p.y).is_none() {
                        s.dragging = false;
                        s.moved = true;
                        let status = self.mouse("release", "left", p.x, p.y, sink, ctx);
                        if status != 0 { return status; }
                    } else {
                        let status = self.mouse("move", "", p.x, p.y, sink, ctx);
                        if status != 0 { return status; }
                    }
                }
            }
            if action == "up" {
                if s.dragging {
                    let status = self.mouse("release", "left", p.x, p.y, sink, ctx);
                    if status != 0 { return status; }
                } else if s.hit && !s.moved && !s.ready && time.saturating_sub(s.time) < 1000 {
                    let status = self.mouse("press", "left", p.x, p.y, sink, ctx);
                    if status != 0 { return status; }
                    let status = self.mouse("release", "left", p.x, p.y, sink, ctx);
                    if status != 0 { return status; }
                }
                self.gesture.next_tick = 0;
            }
            if action != "up" {
                self.gesture.single = Some(s);
            }
        }
        0
    }
    fn cancel_input(&mut self, sink: ControllerInputSink, ctx: *mut c_void) -> i32 {
        let status = self.release_with(sink, ctx);
        self.remote_held = false;
        status
    }
    fn pointer_touch(
        &mut self,
        action: &str,
        points: &[Pt],
        changed: &[Pt],
        time: u64,
        sink: ControllerInputSink,
        ctx: *mut c_void,
    ) -> i32 {
        if action == "down" && points.len() == 1 {
            let p = points[0];
            self.gesture.single = Some(Single {
                start: p,
                last: p,
                time,
                hit: self.in_safe(p.x, p.y),
                ..Default::default()
            });
            self.gesture.next_tick = if self.enabled && self.gesture.single.as_ref().map_or(false, |s| s.hit) {
                time.saturating_add(1000)
            } else { 0 };
            return 0;
        }
        if let Some(mut s) = self.gesture.single.take() {
            let p = changed
                .iter().find(|p| p.id == s.start.id)
                .copied()
                .or_else(|| points.iter().find(|p| p.id == s.start.id).copied())
                .unwrap_or(s.last);
            if action == "move" {
                let distance = ((p.x - s.start.x).powi(2) + (p.y - s.start.y).powi(2)).sqrt();
                if !self.enabled {
                    if s.hit && distance > PAN_THRESHOLD && self.zoom > 1.0 {
                        self.image_x += p.x - s.last.x;
                        self.image_y += p.y - s.last.y;
                        self.clamp_image();
                    }
                    s.moved = distance > PAN_THRESHOLD;
                    s.last = p;
                    self.gesture.single = Some(s);
                    return 0;
                }
                if s.ready && !s.dragging && s.hit && self.enabled && (p.x != s.last.x || p.y != s.last.y) {
                    let status = self.pointer_mouse("press", "left", sink, ctx);
                    if status != 0 { return status; }
                    s.dragging = true;
                }
                if !s.ready && distance > PAN_THRESHOLD {
                    s.moved = true;
                    self.gesture.next_tick = 0;
                }
                if !self.in_safe(p.x, p.y) && s.dragging {
                    self.cursor_x = (self.cursor_x + (p.x.clamp(self.safe_x, self.safe_x + self.safe_w) - s.last.x) / self.scale()).clamp(0.0, self.remote_w);
                    self.cursor_y = (self.cursor_y + (p.y.clamp(self.safe_y, self.safe_y + self.safe_h) - s.last.y) / self.scale()).clamp(0.0, self.remote_h);
                    self.gesture = Gesture::default();
                    return self.pointer_mouse("release", "left", sink, ctx);
                }
                let old_x = self.cursor_x;
                let old_y = self.cursor_y;
                if s.hit {
                    self.cursor_x = (self.cursor_x + (p.x - s.last.x) / self.scale()).clamp(0.0, self.remote_w);
                    self.cursor_y = (self.cursor_y + (p.y - s.last.y) / self.scale()).clamp(0.0, self.remote_h);
                }
                let dx = self.cursor_x.round() as i32 - old_x.round() as i32;
                let dy = self.cursor_y.round() as i32 - old_y.round() as i32;
                if s.hit && self.enabled && (dx != 0 || dy != 0) {
                    let status = self.pointer_mouse("move", "", sink, ctx);
                    if status != 0 { return status; }
                }
                s.last = p;
            }
            if action == "up" {
                self.gesture.next_tick = 0;
                if s.dragging {
                    return self.pointer_mouse("release", "left", sink, ctx);
                }
                if s.hit && !s.moved && !s.ready && time.saturating_sub(s.time) < 1000 {
                    let status = self.pointer_mouse("press", "left", sink, ctx);
                    if status != 0 { return status; }
                    return self.pointer_mouse("release", "left", sink, ctx);
                }
            }
            if action != "up" {
                self.gesture.single = Some(s);
            }
        }
        0
    }
    fn tick(&mut self, time: u64, _sink: ControllerInputSink, _ctx: *mut c_void) -> i32 {
        if self.gesture.single.is_none() {
            self.gesture.next_tick = 0;
        }
        if let Some(mut s) = self.gesture.single.take() {
            if self.enabled && s.hit && !s.ready && time >= s.time.saturating_add(1000) && !s.moved {
                s.ready = true;
                self.gesture.next_tick = 0;
            }
            self.gesture.single = Some(s);
        }
        0
    }
    fn event(&mut self, v: Value, sink: ControllerInputSink, ctx: *mut c_void) -> i32 {
        let Some(kind) = v.get("kind").and_then(Value::as_str) else {
            return 1;
        };
        let allowed = match kind {
            "configure" => &[
                "kind",
                "viewportWidth",
                "viewportHeight",
                "remoteWidth",
                "remoteHeight",
                "insets",
                "padding",
                "fullViewport",
                "enabled",
            ][..],
            "touch" => &["kind", "action", "points", "changedPoints", "time"][..],
            "tick" => &["kind", "time"][..],
            "touch_mode" => &["kind", "mode"][..],
            "reset_view" | "release" => &["kind"][..],
            "mouse" => &["kind", "action", "button", "x", "y"][..],
            "wheel" => &["kind", "action", "x", "y", "dx", "dy", "discrete"][..],
            "key" => &["kind", "action", "physicalCode", "code"][..],
            "text" => &["kind", "text"][..],
            _ => return 1,
        };
        if !Self::fields_only(&v, allowed) {
            return 1;
        }
        match kind {
            "configure" => {
                serde_json::from_value(v).map_or(1, |c| self.configure(c, Some(sink), ctx))
            }
            "touch" => self.touch(&v, sink, ctx),
            "tick" => {
                let Some(time) = v.get("time").and_then(Value::as_u64) else {
                    return 1;
                };
                self.tick(time, sink, ctx)
            }
            "reset_view" => {
                let status = self.cancel(Some(sink), ctx);
                if status != 0 { return status; }
                self.zoom = 1.0;
                self.image_x = self.safe_x + (self.safe_w - self.image_w()) / 2.0;
                self.image_y = self.safe_y + (self.safe_h - self.image_h()) / 2.0;
                self.clamp_image();
                0
            }
            "touch_mode" => match v.get("mode").and_then(Value::as_str) {
                Some("pointer") => {
                    if self.touch_mode {
                        return 0;
                    }
                    let status = self.cancel(Some(sink), ctx);
                    if status != 0 { return status; }
                    self.touch_mode = true;
                    self.initialize_pointer(Some(sink), ctx)
                }
                Some("direct") => {
                    if self.touch_mode {
                        let status = self.cancel(Some(sink), ctx);
                        if status != 0 { return status; }
                    }
                    self.touch_mode = false;
                    0
                }
                _ => 1,
            },
            "mouse" => {
                if !self.enabled {
                    return 0;
                };
                let x = v.get("x").and_then(Value::as_f64).unwrap_or(f64::NAN);
                let y = v.get("y").and_then(Value::as_f64).unwrap_or(f64::NAN);
                let action = v.get("action").and_then(Value::as_str).unwrap_or("");
                let button = v.get("button").and_then(Value::as_str).unwrap_or("");
                if !matches!(action, "press" | "release" | "move" | "cancel")
                    || !matches!(button, "left" | "right" | "middle" | "")
                    || !x.is_finite()
                    || !y.is_finite()
                {
                    1
                } else {
                    self.mouse(action, button, x, y, sink, ctx)
                }
            }
            "wheel" => {
                if !self.enabled {
                    return 0;
                };
                let action = v.get("action").and_then(Value::as_str).unwrap_or("");
                let x = v.get("x").and_then(Value::as_f64).unwrap_or(f64::NAN);
                let y = v.get("y").and_then(Value::as_f64).unwrap_or(f64::NAN);
                let dx = v.get("dx").and_then(Value::as_f64).unwrap_or(f64::NAN);
                let dy = v.get("dy").and_then(Value::as_f64).unwrap_or(f64::NAN);
                let discrete = v.get("discrete").and_then(Value::as_bool).unwrap_or(false);
                if !matches!(action, "begin" | "update" | "")
                    || [x, y, dx, dy].iter().any(|n| !n.is_finite())
                {
                    1
                } else {
                    self.wheel(x, y, dx, dy, discrete, sink, ctx)
                }
            }
            "key" | "text" => {
                if !self.enabled {
                    return 0;
                };
                self.input_event(v, sink, ctx)
            }
            "release" => {
                self.cancel(Some(sink), ctx)
            }
            _ => 1,
        }
    }
    fn state(&self) -> Value {
        let gesture = if self.gesture.single.as_ref().map_or(false, |s| s.dragging) {
            "dragging"
        } else if self.gesture.single.as_ref().map_or(false, |s| s.ready) {
            "dragReady"
        } else if self.gesture.multi.as_ref().map_or(false, |m| m.pinch) {
            "pinch"
        } else if self.gesture.multi.as_ref().map_or(false, |m| m.scroll) {
            "scroll"
        } else if self.gesture.multi.is_some() {
            "multi"
        } else {
            "idle"
        };
        json!({"zoom":self.zoom,"imageX":self.image_x,"imageY":self.image_y,"imageWidth":self.image_w(),"imageHeight":self.image_h(),"safeX":self.safe_x,"safeY":self.safe_y,"safeWidth":self.safe_w,"safeHeight":self.safe_h,"dragReady":self.gesture.single.as_ref().map_or(false,|s|s.ready),"resetVisible":self.zoom>1.0,"touchMode":if self.touch_mode{"pointer"}else{"direct"},"inputEnabled":self.enabled && self.valid,"gesture":gesture,"nextTickMs":self.gesture.next_tick})
    }
}

#[no_mangle]
pub extern "C" fn controller_canvas_new_v1() -> *mut ControllerCanvas {
    Box::into_raw(Box::new(ControllerCanvas::new()))
}

#[no_mangle]
pub extern "C" fn controller_canvas_event_v1(
    canvas: *mut ControllerCanvas,
    event_json: *const c_char,
    sink: Option<ControllerInputSink>,
    ctx: *mut c_void,
) -> i32 {
    if canvas.is_null() || event_json.is_null() {
        return 1;
    }
    let Some(sink) = sink else { return 1 };
    let raw = unsafe { CStr::from_ptr(event_json) }.to_bytes();
    if raw.len() > MAX_EVENT_BYTES {
        return 1;
    }
    let Ok(v) = serde_json::from_slice::<Value>(raw) else {
        return 1;
    };
    unsafe { (*canvas).event(v, sink, ctx) }
}

#[no_mangle]
pub extern "C" fn controller_canvas_state_v1(canvas: *mut ControllerCanvas) -> *mut c_char {
    if canvas.is_null() {
        return ptr::null_mut();
    }
    CString::new(unsafe { (*canvas).state().to_string() })
        .map_or(ptr::null_mut(), CString::into_raw)
}

#[no_mangle]
pub extern "C" fn controller_canvas_suspend_v1(canvas: *mut ControllerCanvas) {
    if let Some(canvas) = unsafe { canvas.as_mut() } {
        canvas.suspend();
    }
}

#[no_mangle]
pub extern "C" fn controller_canvas_free_v1(canvas: *mut ControllerCanvas) {
    if !canvas.is_null() {
        unsafe {
            let c = Box::from_raw(canvas);
            controller_input_free_v1(c.input);
        }
    }
}
