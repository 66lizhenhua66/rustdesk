use std::ffi::{c_char, c_void, CStr, CString};

use remote_controller_core::canvas::{
    controller_canvas_event_v1, controller_canvas_free_v1, controller_canvas_new_v1,
    controller_canvas_state_v1, controller_canvas_suspend_v1,
};
use remote_controller_core::controller_free_string;
use serde_json::{json, Value};

#[derive(Default)]
struct Commands {
    values: Vec<Value>,
    fail_at: Option<usize>,
}
unsafe extern "C" fn capture(ctx: *mut c_void, raw: *const c_char) -> i32 {
    let c = &mut *(ctx as *mut Commands);
    c.values.push(serde_json::from_str(CStr::from_ptr(raw).to_str().unwrap()).unwrap());
    if c.fail_at == Some(c.values.len()) { 73 } else { 0 }
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
         .values
        .iter()
        .any(|v| v["kind"] == "button" && v["button"] == "left"));
    c.1.values.clear();
    c.event(json!({"kind":"touch","action":"down","points":[{"id":1,"x":25,"y":25}],"changedPoints":[{"id":1,"x":25,"y":25}],"time":200}));
    c.event(json!({"kind":"touch","action":"up","points":[],"changedPoints":[{"id":1,"x":25,"y":25}],"time":250}));
    assert!(c.1.values.is_empty());
}

#[test]
fn disabled_input_still_allows_view_gesture_and_releases() {
    let mut c = Canvas::new();
    c.event(config(false));
    c.event(json!({"kind":"touch","action":"down","points":[{"id":1,"x":200,"y":150}],"changedPoints":[{"id":1,"x":200,"y":150}],"time":0}));
    c.event(json!({"kind":"touch","action":"move","points":[{"id":1,"x":220,"y":150}],"changedPoints":[{"id":1,"x":220,"y":150}],"time":20}));
    assert!(c.1.values.is_empty());
    assert_eq!(c.state()["inputEnabled"], false);
}

fn touch(action: &str, points: &[(i64, f64, f64)], changed: &[(i64, f64, f64)], time: u64) -> Value {
    let points: Vec<_> = points.iter().map(|(id, x, y)| json!({"id":id,"x":x,"y":y})).collect();
    let changed: Vec<_> = changed.iter().map(|(id, x, y)| json!({"id":id,"x":x,"y":y})).collect();
    json!({"kind":"touch","action":action,"points":points,"changedPoints":changed,"time":time})
}

#[test]
fn finite_wheel_reaches_sink() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"wheel","action":"update","x":200,"y":150,"dx":0,"dy":2,"discrete":true})), 0);
    assert!(c.1.values.iter().any(|v| v == &json!({"kind":"wheel","dx":0,"dy":2})));
}

#[test]
fn sink_failure_stops_tap_and_disables_input() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    c.1.values.clear();
    c.1.fail_at = Some(2);
    assert_eq!(c.event(touch("down", &[(1, 200.0, 150.0)], &[(1, 200.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 200.0, 150.0)], 50)), 73);
    assert_eq!(c.1.values.len(), 2);
    assert_eq!(c.state()["inputEnabled"], false);
    assert_eq!(c.state()["gesture"], "idle");
}

#[test]
fn release_propagates_failure_and_disabling_preserves_view() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(json!({"kind":"mouse","action":"press","button":"left","x":200,"y":150})), 0);
    c.1.values.clear();
    c.1.fail_at = Some(1);
    assert_eq!(c.event(config(false)), 73);
    assert_eq!(c.1.values, vec![json!({"kind":"release_all"})]);
    assert_eq!(c.state()["inputEnabled"], false);
    assert_eq!(c.state()["zoom"], 1.0);
}

#[test]
fn pinch_and_rotation_keep_normalized_safe_center() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(false)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0)], &[(1, 150.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0), (2, 250.0, 150.0)], &[(2, 250.0, 150.0)], 10)), 0);
    assert_eq!(c.event(touch("move", &[(1, 100.0, 150.0), (2, 300.0, 150.0)], &[(1, 100.0, 150.0), (2, 300.0, 150.0)], 30)), 0);
    assert_eq!(c.state()["zoom"], 2.0);
    assert_eq!(c.event(touch("up", &[(1, 100.0, 150.0)], &[(2, 300.0, 150.0)], 40)), 0);
    let before = c.state();
    assert_eq!(c.event(touch("move", &[(1, 130.0, 150.0)], &[(1, 130.0, 150.0)], 50)), 0);
    let panned = c.state();
    assert!((panned["imageX"].as_f64().unwrap() - before["imageX"].as_f64().unwrap() - 30.0).abs() < 0.01);
    let center_fraction = |s: &Value| ((s["safeX"].as_f64().unwrap() + s["safeWidth"].as_f64().unwrap() / 2.0 - s["imageX"].as_f64().unwrap()) / s["imageWidth"].as_f64().unwrap(), (s["safeY"].as_f64().unwrap() + s["safeHeight"].as_f64().unwrap() / 2.0 - s["imageY"].as_f64().unwrap()) / s["imageHeight"].as_f64().unwrap());
    let old_center = center_fraction(&panned);
    assert_eq!(c.event(json!({"kind":"configure","viewportWidth":500,"viewportHeight":320,"remoteWidth":400,"remoteHeight":200,"enabled":false})), 0);
    let rotated = c.state();
    let new_center = center_fraction(&rotated);
    assert_eq!(rotated["zoom"], 2.0);
    assert!((old_center.0 - new_center.0).abs() < 0.01);
    assert!((old_center.1 - new_center.1).abs() < 0.01);
    assert!(c.1.values.is_empty());
}

#[test]
fn long_press_drag_and_two_finger_gestures_do_not_misclick() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 200.0, 150.0)], &[(1, 200.0, 150.0)], 0)), 0);
    assert_eq!(c.event(json!({"kind":"tick","time":1000})), 0);
    assert_eq!(c.state()["gesture"], "dragReady");
    assert!(c.1.values.is_empty());
    assert_eq!(c.event(touch("move", &[(1, 215.0, 150.0)], &[(1, 215.0, 150.0)], 1100)), 0);
    assert_eq!(c.state()["gesture"], "dragging");
    assert_eq!(c.event(touch("up", &[], &[(1, 215.0, 150.0)], 1200)), 0);
    assert_eq!(c.1.values.iter().filter(|v| v["kind"] == "button" && v["button"] == "left").count(), 2);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0)], &[(1, 180.0, 150.0)], 1300)), 0);
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0), (2, 220.0, 150.0)], &[(2, 220.0, 150.0)], 1320)), 0);
    assert_eq!(c.state()["nextTickMs"], 0);
    assert_eq!(c.event(touch("up", &[(1, 180.0, 150.0)], &[(2, 220.0, 150.0)], 1380)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 180.0, 150.0)], 1400)), 0);
    assert_eq!(c.1.values.iter().filter(|v| v["kind"] == "button" && v["button"] == "right").count(), 2);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0)], &[(1, 180.0, 150.0)], 1500)), 0);
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0), (2, 220.0, 150.0)], &[(2, 220.0, 150.0)], 1520)), 0);
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0), (2, 220.0, 150.0), (3, 200.0, 180.0)], &[(3, 200.0, 180.0)], 1540)), 0);
    assert_eq!(c.state()["nextTickMs"], 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 180.0, 150.0)], 1580)), 0);
    assert!(c.1.values.iter().all(|v| v["kind"] != "button"));
}

#[test]
fn read_only_pan_has_no_drag_timer_or_remote_commands() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(false)), 0);
    assert_eq!(c.event(touch("down", &[(1, 200.0, 150.0)], &[(1, 200.0, 150.0)], 0)), 0);
    assert_eq!(c.state()["nextTickMs"], 0);
    assert_eq!(c.event(json!({"kind":"tick","time":1100})), 0);
    assert_eq!(c.state()["dragReady"], false);
    assert_eq!(c.event(touch("up", &[], &[(1, 200.0, 150.0)], 1200)), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn pointer_mode_moves_to_absolute_target_and_keeps_pinch_local() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 200.0, 150.0)], &[(1, 200.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("move", &[(1, 212.0, 150.0)], &[(1, 212.0, 150.0)], 20)), 0);
    assert_eq!(c.1.values, vec![json!({"kind":"move","x":35002,"y":32768})]);
    assert_eq!(c.event(touch("up", &[], &[(1, 212.0, 150.0)], 40)), 0);
    assert!(c.1.values.iter().all(|v| v["kind"] != "button"));
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0)], &[(1, 150.0, 150.0)], 100)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0), (2, 250.0, 150.0)], &[(2, 250.0, 150.0)], 110)), 0);
    assert_eq!(c.event(touch("move", &[(1, 100.0, 150.0), (2, 300.0, 150.0)], &[(1, 100.0, 150.0), (2, 300.0, 150.0)], 120)), 0);
    assert_eq!(c.state()["zoom"], 2.0);
    assert!(c.1.values.iter().all(|v| v["kind"] != "button" && v["kind"] != "wheel"));
}

#[test]
fn pointer_mode_initializes_at_visible_center_only_when_selected() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0), (2, 250.0, 150.0)], &[], 0)), 0);
    assert_eq!(c.event(touch("move", &[(1, 100.0, 150.0), (2, 300.0, 150.0)], &[], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[], 30)), 0);
    assert_eq!(c.event(touch("down", &[(3, 200.0, 150.0)], &[], 40)), 0);
    assert_eq!(c.event(touch("move", &[(3, 235.2, 150.0)], &[], 60)), 0);
    assert_eq!(c.event(touch("up", &[], &[(3, 235.2, 150.0)], 80)), 0);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    assert_eq!(c.1.values, vec![json!({"kind":"move","x":29491,"y":32768})]);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn pointer_slide_from_letterbox_and_tap_use_the_same_target() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 100.0, 40.0)], &[], 0)), 0);
    assert_eq!(c.event(touch("move", &[(1, 135.2, 40.0)], &[], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 135.2, 40.0)], 40)), 0);
    let target = json!({"kind":"move","x":39321,"y":32768});
    assert_eq!(c.1.values, vec![target.clone()]);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(2, 300.0, 260.0)], &[], 100)), 0);
    assert_eq!(c.event(touch("up", &[], &[(2, 300.0, 260.0)], 150)), 0);
    assert_eq!(c.1.values, vec![target.clone(), json!({"kind":"button","button":"left","down":true}),
        target, json!({"kind":"button","button":"left","down":false})]);
}

#[test]
fn pointer_permission_grant_initializes_without_resetting_later_configures() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(false)), 0);
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    assert!(c.1.values.is_empty());
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.1.values, vec![json!({"kind":"move","x":32768,"y":32768})]);
    assert_eq!(c.event(touch("down", &[(1, 200.0, 150.0)], &[], 0)), 0);
    assert_eq!(c.event(touch("move", &[(1, 235.2, 150.0)], &[], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 235.2, 150.0)], 40)), 0);
    c.1.values.clear();
    assert_eq!(c.event(config(true)), 0);
    let mut resized = config(true);
    resized["viewportWidth"] = json!(450);
    assert_eq!(c.event(resized), 0);
    assert!(c.1.values.is_empty());
    assert_eq!(c.event(touch("down", &[(2, 200.0, 150.0)], &[], 100)), 0);
    assert_eq!(c.event(touch("up", &[], &[(2, 200.0, 150.0)], 150)), 0);
    assert_eq!(c.1.values[0], json!({"kind":"move","x":39321,"y":32768}));
    c.1.values.clear();
    assert_eq!(c.event(config(false)), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn two_finger_scroll_does_not_right_click() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0)], &[(1, 180.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0), (2, 220.0, 150.0)], &[(2, 220.0, 150.0)], 10)), 0);
    assert_eq!(c.event(touch("move", &[(1, 180.0, 180.0), (2, 220.0, 180.0)], &[(1, 180.0, 180.0), (2, 220.0, 180.0)], 30)), 0);
    assert!(c.1.values.iter().any(|v| v == &json!({"kind":"wheel","dx":0,"dy":1})));
    assert_eq!(c.event(touch("up", &[], &[(1, 180.0, 180.0)], 50)), 0);
    assert!(c.1.values.iter().all(|v| v["kind"] != "button"));
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0)], &[(1, 180.0, 150.0)], 100)), 0);
    assert_eq!(c.event(touch("down", &[(1, 180.0, 150.0), (2, 220.0, 150.0)], &[(2, 220.0, 150.0)], 110)), 0);
    assert_eq!(c.event(touch("move", &[(1, 180.0, 162.0), (2, 220.0, 162.0)], &[(1, 180.0, 162.0), (2, 220.0, 162.0)], 120)), 0);
    assert_eq!(c.event(touch("up", &[(1, 180.0, 162.0)], &[(2, 220.0, 162.0)], 140)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 180.0, 162.0)], 150)), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn suspend_clears_local_hold_without_sink_and_keeps_view() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0)], &[(1, 150.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0), (2, 250.0, 150.0)], &[(2, 250.0, 150.0)], 10)), 0);
    assert_eq!(c.event(touch("move", &[(1, 100.0, 150.0), (2, 300.0, 150.0)], &[(1, 100.0, 150.0), (2, 300.0, 150.0)], 20)), 0);
    assert_eq!(c.state()["zoom"], 2.0);
    assert_eq!(c.event(json!({"kind":"mouse","action":"press","button":"left","x":200,"y":150})), 0);
    c.1.values.clear();
    controller_canvas_suspend_v1(c.0);
    assert!(c.1.values.is_empty());
    assert_eq!(c.state()["inputEnabled"], false);
    assert_eq!(c.state()["zoom"], 2.0);
    assert_eq!(c.state()["nextTickMs"], 0);
    assert_eq!(c.event(json!({"kind":"release"})), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn reset_view_releases_keyboard_and_mouse_hold_but_keeps_authorization() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(json!({"kind":"key","action":"down","physicalCode":42,"code":"Shift"})), 0);
    assert_eq!(c.event(json!({"kind":"mouse","action":"press","button":"left","x":200,"y":150})), 0);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"reset_view"})), 0);
    assert_eq!(c.1.values, vec![json!({"kind":"release_all"})]);
    assert_eq!(c.state()["inputEnabled"], true);
    assert_eq!(c.state()["zoom"], 1.0);
    assert_eq!(c.state()["nextTickMs"], 0);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"release"})), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn zoomed_image_outside_safe_rect_cannot_click() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0)], &[(1, 150.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0), (2, 250.0, 150.0)], &[(2, 250.0, 150.0)], 10)), 0);
    assert_eq!(c.event(touch("move", &[(1, 100.0, 150.0), (2, 300.0, 150.0)], &[(1, 100.0, 150.0), (2, 300.0, 150.0)], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 100.0, 150.0)], 30)), 0);
    assert_eq!(c.state()["zoom"], 2.0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(3, 10.0, 150.0)], &[(3, 10.0, 150.0)], 100)), 0);
    assert_eq!(c.event(touch("up", &[], &[(3, 10.0, 150.0)], 150)), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn shared_canvas_event_fixture_matches_public_abi() {
    let fixture: Value = serde_json::from_str(include_str!("../../../apps/harmony-controller/tests/fixtures/canvas-events.json")).unwrap();
    let mut c = Canvas::new();
    for event in fixture["events"].as_array().unwrap() {
        assert_eq!(c.event(event.clone()), 0);
    }
    assert_eq!(c.1.values, fixture["expectedCommands"].as_array().unwrap().clone());
}

#[test]
fn read_only_pointer_mode_can_pan_zoomed_canvas() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(false)), 0);
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0)], &[(1, 150.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 150.0, 150.0), (2, 250.0, 150.0)], &[(2, 250.0, 150.0)], 10)), 0);
    assert_eq!(c.event(touch("move", &[(1, 100.0, 150.0), (2, 300.0, 150.0)], &[(1, 100.0, 150.0), (2, 300.0, 150.0)], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 100.0, 150.0)], 30)), 0);
    let before = c.state()["imageX"].as_f64().unwrap();
    assert_eq!(c.event(touch("down", &[(3, 200.0, 150.0)], &[(3, 200.0, 150.0)], 100)), 0);
    assert_eq!(c.event(touch("move", &[(3, 220.0, 150.0)], &[(3, 220.0, 150.0)], 120)), 0);
    assert!((c.state()["imageX"].as_f64().unwrap() - before - 20.0).abs() < 0.01);
    assert!(c.1.values.is_empty());
}

#[test]
fn pointer_drag_releases_at_safe_edge() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 200.0, 150.0)], &[(1, 200.0, 150.0)], 0)), 0);
    assert_eq!(c.event(json!({"kind":"tick","time":1000})), 0);
    assert_eq!(c.event(touch("move", &[(1, 210.0, 150.0)], &[(1, 210.0, 150.0)], 1100)), 0);
    assert_eq!(c.state()["gesture"], "dragging");
    assert_eq!(c.event(touch("move", &[(1, 410.0, 150.0)], &[(1, 410.0, 150.0)], 1200)), 0);
    assert_eq!(c.state()["gesture"], "idle");
    assert_eq!(c.1.values.iter().filter(|v| v["kind"] == "button" && v["button"] == "left").count(), 2);
}

#[test]
fn pointer_two_finger_tap_in_padding_does_not_click() {
    let mut c = Canvas::new();
    assert_eq!(c.event(config(true)), 0);
    assert_eq!(c.event(json!({"kind":"touch_mode","mode":"pointer"})), 0);
    c.1.values.clear();
    assert_eq!(c.event(touch("down", &[(1, 10.0, 150.0)], &[(1, 10.0, 150.0)], 0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 10.0, 150.0), (2, 20.0, 150.0)], &[(2, 20.0, 150.0)], 10)), 0);
    assert_eq!(c.event(touch("up", &[], &[(1, 10.0, 150.0)], 100)), 0);
    assert!(c.1.values.is_empty());
}

fn full_viewport_config(width: f64, height: f64) -> Value {
    json!({"kind":"configure","viewportWidth":width,"viewportHeight":height,
        "remoteWidth":1920,"remoteHeight":1080,"fullViewport":true,
        "insets":{"left":40,"top":0,"right":0,"bottom":16},"padding":24,"enabled":true})
}

#[test]
fn full_viewport_fit_uses_screen_even_when_input_insets_change() {
    let mut c = Canvas::new();
    let mut config = full_viewport_config(809.0, 376.0);
    assert_eq!(c.event(config.clone()), 0);
    let fitted = c.state();
    let width = 376.0 * 16.0 / 9.0;
    assert!((fitted["imageWidth"].as_f64().unwrap() - width).abs() < 0.01);
    assert!((fitted["imageX"].as_f64().unwrap() - (809.0 - width) / 2.0).abs() < 0.01);
    assert_eq!(fitted["imageY"], 0.0);
    assert_eq!(fitted["imageHeight"], 376.0);
    assert_eq!(fitted["safeX"], 64.0);
    config["insets"] = json!({"left":80,"top":32,"right":8,"bottom":24});
    assert_eq!(c.event(config.clone()), 0);
    let inset = c.state();
    for field in ["imageX", "imageY", "imageWidth", "imageHeight"] {
        assert_eq!(inset[field], fitted[field]);
    }
    assert_eq!(inset["safeX"], 104.0);
    config["padding"] = json!(500);
    assert_eq!(c.event(config), 0);
    assert_eq!(c.state()["inputEnabled"], false);
    assert_eq!(c.state()["imageHeight"], 376.0);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"text","text":"blocked"})), 0);
    assert!(c.1.values.is_empty());
}

#[test]
fn full_viewport_rotation_preserves_zoom_and_screen_center_until_reset() {
    let mut c = Canvas::new();
    assert_eq!(c.event(full_viewport_config(809.0, 376.0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 354.5, 188.0), (2, 454.5, 188.0)], &[], 0)), 0);
    assert_eq!(c.event(touch("move", &[(1, 304.5, 188.0), (2, 504.5, 188.0)], &[], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[], 30)), 0);
    assert_eq!(c.event(touch("down", &[(3, 404.5, 188.0)], &[], 40)), 0);
    assert_eq!(c.event(touch("move", &[(3, 454.5, 198.0)], &[], 60)), 0);
    let before = c.state();
    let center_x = (404.5 - before["imageX"].as_f64().unwrap()) / before["imageWidth"].as_f64().unwrap();
    let center_y = (188.0 - before["imageY"].as_f64().unwrap()) / before["imageHeight"].as_f64().unwrap();
    assert_eq!(c.event(full_viewport_config(376.0, 809.0)), 0);
    let rotated = c.state();
    assert_eq!(rotated["zoom"], 2.0);
    assert!(((188.0 - rotated["imageX"].as_f64().unwrap()) / rotated["imageWidth"].as_f64().unwrap() - center_x).abs() < 0.001);
    assert!(((404.5 - rotated["imageY"].as_f64().unwrap()) / rotated["imageHeight"].as_f64().unwrap() - center_y).abs() < 0.001);
    assert_eq!(c.event(json!({"kind":"reset_view"})), 0);
    let reset = c.state();
    assert_eq!(reset["zoom"], 1.0);
    assert_eq!(reset["imageX"], 0.0);
    assert_eq!(reset["imageWidth"], 376.0);
    assert!((reset["imageY"].as_f64().unwrap() - (809.0 - 376.0 * 9.0 / 16.0) / 2.0).abs() < 0.01);
}

#[test]
fn full_viewport_pan_exposes_remote_edges_inside_input_safe_rect() {
    let mut c = Canvas::new();
    assert_eq!(c.event(full_viewport_config(809.0, 376.0)), 0);
    assert_eq!(c.event(touch("down", &[(1, 354.5, 188.0), (2, 454.5, 188.0)], &[], 0)), 0);
    assert_eq!(c.event(touch("move", &[(1, 304.5, 188.0), (2, 504.5, 188.0)], &[], 20)), 0);
    assert_eq!(c.event(touch("up", &[], &[], 30)), 0);
    assert_eq!(c.event(touch("down", &[(3, 404.5, 188.0)], &[], 40)), 0);
    assert_eq!(c.event(touch("move", &[(3, 3000.0, 3000.0)], &[], 60)), 0);
    assert_eq!(c.event(touch("up", &[], &[(3, 3000.0, 3000.0)], 70)), 0);
    let edge = c.state();
    let x = edge["imageX"].as_f64().unwrap();
    let y = edge["imageY"].as_f64().unwrap();
    assert_eq!(x, edge["safeX"].as_f64().unwrap() + 24.0);
    assert_eq!(y, edge["safeY"].as_f64().unwrap() + 24.0);
    c.1.values.clear();
    assert_eq!(c.event(json!({"kind":"mouse","action":"press","button":"left","x":x - 12.0,"y":y + 12.0})), 0);
    assert_eq!(c.event(json!({"kind":"mouse","action":"press","button":"left","x":x + 12.0,"y":y - 12.0})), 0);
    assert!(c.1.values.is_empty());
    assert_eq!(c.event(touch("down", &[(4, x + 1.0, y + 1.0)], &[], 100)), 0);
    assert_eq!(c.event(touch("up", &[], &[(4, x + 1.0, y + 1.0)], 150)), 0);
    assert_eq!(c.1.values.iter().filter(|v| v["kind"] == "button" && v["button"] == "left").count(), 2);
}
