use std::ffi::{c_char, c_void, CStr, CString};

use remote_controller_core::interaction::{
    controller_input_event_v1, controller_input_free_v1, controller_input_new_v1,
    controller_input_reset_v1, ControllerInput,
};
use serde_json::{json, Value};

#[derive(Default)]
struct Commands {
    values: Vec<Value>,
    fail_at: Option<usize>,
}

unsafe extern "C" fn capture(context: *mut c_void, command: *const c_char) -> i32 {
    let output = &mut *(context as *mut Commands);
    output
        .values
        .push(serde_json::from_str(CStr::from_ptr(command).to_str().unwrap()).unwrap());
    if output.fail_at == Some(output.values.len()) {
        4
    } else {
        0
    }
}

struct Engine(*mut ControllerInput, Commands);

impl Engine {
    fn new() -> Self {
        Self(controller_input_new_v1(), Commands::default())
    }

    fn event(&mut self, event: Value) -> i32 {
        let event = CString::new(event.to_string()).unwrap();
        controller_input_event_v1(
            self.0,
            event.as_ptr(),
            Some(capture),
            &mut self.1 as *mut _ as *mut c_void,
        )
    }

    fn take(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.1.values)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        controller_input_free_v1(self.0);
    }
}

fn touch(action: &str, points: Value, changed: Value, time: u64) -> Value {
    json!({"kind":"touch","action":action,"points":points,"changedPoints":changed,
        "time":time,"width":100,"height":100})
}

fn point(x: i32, y: i32) -> Value {
    json!({"id":1,"x":x,"y":y})
}

fn key(action: &str, physical: u32, code: &str) -> Value {
    json!({"kind":"key","action":action,"physicalCode":physical,"code":code})
}

#[test]
fn touch_taps_drags_and_two_finger_scroll_emit_protocol_commands() {
    let mut input = Engine::new();
    for (duration, button) in [(499, "left"), (500, "right")] {
        assert_eq!(
            input.event(touch(
                "down",
                json!([point(50, 50)]),
                json!([point(50, 50)]),
                1000
            )),
            0
        );
        assert_eq!(
            input.event(touch(
                "up",
                json!([]),
                json!([point(50, 50)]),
                1000 + duration
            )),
            0
        );
        assert_eq!(
            input.take(),
            vec![
                json!({"kind":"move","x":32768,"y":32768}),
                json!({"kind":"move","x":32768,"y":32768}),
                json!({"kind":"button","button":button,"down":true}),
                json!({"kind":"button","button":button,"down":false}),
            ]
        );
    }
    input.event(touch(
        "down",
        json!([point(50, 50)]),
        json!([point(50, 50)]),
        0,
    ));
    input.event(touch(
        "move",
        json!([point(56, 50)]),
        json!([point(56, 50)]),
        1,
    ));
    input.event(touch("up", json!([]), json!([point(-10, 110)]), 2));
    assert_eq!(
        input.take(),
        vec![
            json!({"kind":"move","x":32768,"y":32768}),
            json!({"kind":"button","button":"left","down":true}),
            json!({"kind":"move","x":36700,"y":32768}),
            json!({"kind":"move","x":0,"y":65535}),
            json!({"kind":"button","button":"left","down":false}),
        ]
    );
    let two = |y| json!([{"id":1,"x":40,"y":y},{"id":2,"x":60,"y":y}]);
    input.event(touch("down", two(10), json!([point(40, 10)]), 0));
    input.event(touch("move", two(22), json!([point(40, 22)]), 1));
    input.event(touch("move", two(34), json!([point(40, 34)]), 2));
    assert_eq!(input.take(), vec![json!({"kind":"wheel","dx":0,"dy":1})]);
}

#[test]
fn mouse_keyboard_wheel_and_cancel_share_pressed_state_and_release() {
    let mut input = Engine::new();
    input.event(key("down", 1, "Shift"));
    input.event(key("down", 2, "Shift"));
    input.event(key("up", 1, "Shift"));
    assert_eq!(
        input.take(),
        vec![json!({"kind":"key","code":"Shift","down":true}); 2]
    );
    input.event(key("up", 2, "Shift"));
    assert_eq!(
        input.take(),
        vec![json!({"kind":"key","code":"Shift","down":false})]
    );
    input.event(json!({"kind":"mouse","action":"press","button":"right","x":50,"y":50,"width":100,"height":100}));
    input.take();
    input.event(
        json!({"kind":"mouse","action":"cancel","button":"","x":0,"y":0,"width":100,"height":100}),
    );
    input.event(json!({"kind":"release"}));
    assert_eq!(input.take(), vec![json!({"kind":"release_all"})]);
    for _ in 0..2 {
        input.event(json!({"kind":"wheel","action":"update","x":0,"y":0,"dx":0,"dy":20,"discrete":false,"width":100,"height":100}));
    }
    assert_eq!(
        input.take(),
        vec![
            json!({"kind":"move","x":0,"y":0}),
            json!({"kind":"move","x":0,"y":0}),
            json!({"kind":"wheel","dx":0,"dy":1}),
        ]
    );
    input.event(key("down", 1, "KeyA"));
    input.take();
    input.event(touch("cancel", json!([]), json!([]), 0));
    assert_eq!(input.take(), vec![json!({"kind":"release_all"})]);
    input.event(key("down", 1, "KeyA"));
    input.take();
    controller_input_reset_v1(input.0);
    input.event(json!({"kind":"release"}));
    input.event(key("down", 1, ""));
    input.event(key("down", 1, "Unsupported"));
    assert!(input.take().is_empty());
}

#[test]
fn sink_failure_stops_the_batch_resets_pressed_state_and_preserves_status() {
    let mut input = Engine::new();
    for fail_at in 1..=3 {
        input.1.fail_at = None;
        input.event(touch(
            "down",
            json!([point(50, 50)]),
            json!([point(50, 50)]),
            0,
        ));
        input.take();
        input.1.fail_at = Some(fail_at);
        assert_eq!(
            input.event(touch("up", json!([]), json!([point(50, 50)]), 1)),
            4
        );
        assert_eq!(input.take().len(), fail_at);
        input.1.fail_at = None;
        input.event(json!({"kind":"release"}));
        assert!(input.take().is_empty());
    }
    assert_eq!(input.event(json!({"kind":"unknown"})), 1);
    assert_eq!(input.event(json!({"kind":"release","authorized":true})), 1);
    assert_eq!(
        input.event(touch("down", json!(vec![point(0, 0); 17]), json!([]), 0)),
        1
    );
    assert_eq!(
        input.event(json!({"kind":"key","action":"down","physicalCode":1,"code":"a".repeat(8192)})),
        1
    );
    assert!(input.take().is_empty());
    let text = "你🙂".repeat(150);
    assert_eq!(input.event(json!({"kind":"text","text":text})), 0);
    let parts = input.take();
    assert!(parts
        .iter()
        .all(|part| part["text"].as_str().unwrap().len() <= 512));
    assert_eq!(
        parts
            .iter()
            .map(|part| part["text"].as_str().unwrap())
            .collect::<String>(),
        text
    );
    input.1.fail_at = Some(1);
    assert_eq!(input.event(json!({"kind":"text","text":text})), 4);
    assert_eq!(input.take().len(), 1);
    assert_eq!(input.event(json!({"kind":"text","text":"a\0b"})), 1);
    assert!(input.take().is_empty());
}
