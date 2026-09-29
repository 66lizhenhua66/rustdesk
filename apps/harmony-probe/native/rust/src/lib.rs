use std::{
    ffi::{c_char, c_void},
    sync::{Condvar, Mutex},
    time::Duration,
};

pub type ProbeCallback = extern "C" fn(u32, u32, u32, *mut c_void);

#[repr(C)]
pub struct ProbeTask {
    steps: u32,
    interval: Duration,
    cancelled: Mutex<bool>,
    wake: Condvar,
}

#[no_mangle]
pub extern "C" fn probe_version() -> *const c_char {
    c"harmony-probe/1".as_ptr()
}

#[no_mangle]
pub extern "C" fn probe_create(steps: u32, interval_ms: u32) -> *mut ProbeTask {
    if !(1..=100).contains(&steps) || !(10..=1000).contains(&interval_ms) {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(ProbeTask {
        steps,
        interval: Duration::from_millis(u64::from(interval_ms)),
        cancelled: Mutex::new(false),
        wake: Condvar::new(),
    }))
}

#[no_mangle]
pub extern "C" fn probe_run(
    task: *mut ProbeTask,
    callback: Option<ProbeCallback>,
    user: *mut c_void,
) {
    let (Some(task), Some(callback)) = (unsafe { task.as_ref() }, callback) else {
        return;
    };
    let mut last_step = 0;
    for step in 1..=task.steps {
        let cancelled = task
            .cancelled
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if *cancelled {
            callback(3, last_step, task.steps, user);
            return;
        }
        drop(cancelled);
        callback(1, step, task.steps, user);
        last_step = step;
        if step == task.steps {
            break;
        }
        let cancelled = task
            .cancelled
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let (cancelled, _) = task
            .wake
            .wait_timeout_while(cancelled, task.interval, |value| !*value)
            .unwrap_or_else(|poison| poison.into_inner());
        if *cancelled {
            callback(3, last_step, task.steps, user);
            return;
        }
    }
    callback(2, last_step, task.steps, user);
}

#[no_mangle]
pub extern "C" fn probe_cancel(task: *mut ProbeTask) {
    if let Some(task) = unsafe { task.as_ref() } {
        let mut cancelled = task
            .cancelled
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *cancelled = true;
        task.wake.notify_all();
    }
}

#[no_mangle]
pub extern "C" fn probe_destroy(task: *mut ProbeTask) {
    if !task.is_null() {
        drop(unsafe { Box::from_raw(task) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    extern "C" fn collect(kind: u32, step: u32, total: u32, user: *mut c_void) {
        let events = unsafe { &mut *(user as *mut Vec<(u32, u32, u32)>) };
        events.push((kind, step, total));
    }

    #[test]
    fn completion_reports_ordered_progress_and_terminal_event() {
        let task = probe_create(3, 10);
        assert!(!task.is_null());
        let mut events: Vec<(u32, u32, u32)> = Vec::new();
        probe_run(task, Some(collect), (&mut events as *mut Vec<_>).cast());
        probe_destroy(task);
        assert_eq!(events, vec![(1, 1, 3), (1, 2, 3), (1, 3, 3), (2, 3, 3)]);
    }

    #[test]
    fn cancellation_wakes_waiter_and_reports_cancelled() {
        let task = probe_create(100, 1000);
        assert!(!task.is_null());
        let (tx, rx) = mpsc::channel();
        let address = task as usize;
        let worker = thread::spawn(move || {
            let mut events: Vec<(u32, u32, u32)> = Vec::new();
            probe_run(
                address as *mut ProbeTask,
                Some(collect),
                (&mut events as *mut Vec<_>).cast(),
            );
            tx.send(events).unwrap();
        });
        thread::sleep(Duration::from_millis(30));
        let started = Instant::now();
        probe_cancel(task);
        let events = rx.recv_timeout(Duration::from_millis(250)).unwrap();
        worker.join().unwrap();
        probe_destroy(task);
        assert!(started.elapsed() < Duration::from_millis(250));
        assert_eq!(events.last().map(|event| event.0), Some(3));
    }

    #[test]
    fn invalid_arguments_are_rejected() {
        for (steps, interval) in [(0, 10), (101, 10), (1, 9), (1, 1001)] {
            assert!(probe_create(steps, interval).is_null());
        }
        let valid = probe_create(1, 10);
        assert!(!valid.is_null());
        probe_destroy(valid);
    }
}
