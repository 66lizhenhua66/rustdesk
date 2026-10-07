use std::ffi::{c_char, c_void, CStr, CString};

use remote_controller_core::canvas::{
    controller_canvas_event_v1, controller_canvas_free_v1, controller_canvas_new_v1,
    controller_canvas_state_v1,
};
use remote_controller_core::controller_free_string;
use serde_json::{json, Value};

#[derive(Default)]
struct Commands(Vec<Value>);
unsafe extern "C" fn capture(ctx: *mut c_void, raw: *const c_char) -> i32 {
    let c = &mut *(ctx as *mut Commands);
    c.0.push(serde_json::from_str(CStr::from_ptr(raw).to_str().unwrap()).unwrap());
    0
}
struct Canvas(
    *mut remote_controller_core::canvas::ControllerCanvas,
    Commands,
);
impl Canvas {
    fn new() -> Self {
        Self(controller_canvas_new_v1(), Commands::default())
    }
    fn event(&mut self, v: Value) -> i32 {
        let r = CString::new(v.to_string()).unwrap();
        controller_canvas_event_v1(
            self.0,
            r.as_ptr(),
            Some(capture),
            &mut self.1 as *mut _ as *mut c_void,
        )
    }
    fn state(&mut self) -> Value {
        let p = controller_canvas_state_v1(self.0);
        let v = serde_json::from_str(unsafe { CStr::from_ptr(p).to_str().unwrap() }).unwrap();
        controller_free_string(p);
        v
    }
}
impl Drop for Canvas {
    fn drop(&mut self) {
        controller_canvas_free_v1(self.0)
    }
}

fn config(enabled: bool) -> Value {
    json!({"kind":"configure","viewportWidth":400,"viewportHeight":300,"remoteWidth":200,"remoteHeight":100,"insets":{"left":0,"top":0,"right":0,"bottom":0},"padding":24,"enabled":enabled})
}

#[test]
fn fit_zoom_reset_and_invalid_geometry_are_finite() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    let s = c.state();
    assert_eq!(s["zoom"], 1.0);
    assert_eq!(s["imageWidth"], 352.0);
    assert_eq!(s["imageHeight"], 176.0);
    assert_eq!(s["resetVisible"], false);
    assert_eq!(c.event(json!({"kind":"configure","viewportWidth":0,"viewportHeight":0,"remoteWidth":200,"remoteHeight":100,"enabled":true})),0);
    assert_eq!(c.state()["dragReady"], false);
}

#[test]
fn direct_tap_is_transformed_and_black_bars_do_not_click() {
    let mut c = Canvas::new();
    c.event(config(true));
    c.event(json!({"kind":"touch","action":"down","points":[{"id":1,"x":200,"y":150}],"changedPoints":[{"id":1,"x":200,"y":150}],"time":10}));
    c.event(json!({"kind":"touch","action":"up","points":[],"changedPoints":[{"id":1,"x":200,"y":150}],"time":100}));
    assert!(c
        .1
         .0
        .iter()
        .any(|v| v["kind"] == "button" && v["button"] == "left"));
    c.1 .0.clear();
    c.event(json!({"kind":"touch","action":"down","points":[{"id":1,"x":25,"y":25}],"changedPoints":[{"id":1,"x":25,"y":25}],"time":200}));
    c.event(json!({"kind":"touch","action":"up","points":[],"changedPoints":[{"id":1,"x":25,"y":25}],"time":250}));
    assert!(c.1 .0.is_empty());
}

#[test]
fn disabled_input_still_allows_view_gesture_and_releases() {
    let mut c = Canvas::new();
    c.event(config(false));
    c.event(json!({"kind":"touch","action":"down","points":[{"id":1,"x":200,"y":150}],"changedPoints":[{"id":1,"x":200,"y":150}],"time":0}));
    c.event(json!({"kind":"touch","action":"move","points":[{"id":1,"x":220,"y":150}],"changedPoints":[{"id":1,"x":220,"y":150}],"time":20}));
    assert!(c.1 .0.is_empty());
    assert_eq!(c.state()["inputEnabled"], false);
}
