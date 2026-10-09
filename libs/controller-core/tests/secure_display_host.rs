#[path = "../../../src/server/secure_display_policy.rs"]
mod policy;

use policy::{Backend, DisplayError, Journal, JournalStore, Mode, ModeSession, State};
use std::sync::{atomic::AtomicBool, Arc, Mutex};

#[derive(Clone)]
struct Display(Arc<Mutex<(State, bool)>>);

impl Backend for Display {
    fn current(&mut self) -> Result<State, DisplayError> {
        Ok(self.0.lock().unwrap().0.clone())
    }
    fn modes(&mut self) -> Result<Vec<Mode>, DisplayError> {
        Ok(vec![mode(1920, 1080), mode(1600, 900), mode(1280, 720)])
    }
    fn test(&mut self, _: &Mode) -> Result<(), DisplayError> {
        Ok(())
    }
    fn apply(&mut self, requested: &Mode) -> Result<(), DisplayError> {
        let mut state = self.0.lock().unwrap();
        state.0.mode = requested.clone();
        if std::mem::take(&mut state.1) {
            return Err(DisplayError::Failed("driver reported failure".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Store(Arc<Mutex<Option<Journal>>>);

impl JournalStore for Store {
    fn read(&mut self) -> Result<Option<Journal>, DisplayError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn write(&mut self, journal: &Journal) -> Result<(), DisplayError> {
        *self.0.lock().unwrap() = Some(journal.clone());
        Ok(())
    }
    fn remove(&mut self, owner: &str) -> Result<(), DisplayError> {
        let mut state = self.0.lock().unwrap();
        if state.as_ref().map(|journal| journal.owner.as_str()) == Some(owner) {
            *state = None;
        }
        Ok(())
    }
    fn report_restore_error(&self, _: &DisplayError) {}
}

fn mode(width: u32, height: u32) -> Mode {
    Mode {
        width,
        height,
        frequency: 60,
        bits: 32,
        orientation: 0,
        fixed_output: 0,
        display_flags: 0,
        x: 0,
        y: 0,
    }
}

fn display() -> Display {
    Display(Arc::new(Mutex::new((
        State {
            device: "primary".into(),
            topology: vec!["primary:monitor1".into()],
            mode: mode(1920, 1080),
        },
        false,
    ))))
}

#[test]
fn disconnect_restores_first_original_after_multiple_successful_changes() {
    let mut display = display();
    let store = Store::default();
    let owner = Arc::new(AtomicBool::new(false));
    {
        let mut session =
            ModeSession::open(display.clone(), store.clone(), owner.clone(), "one".into()).unwrap();
        let snapshot = session.snapshot().unwrap();
        assert_eq!(
            (snapshot.original_width, snapshot.original_height),
            (1920, 1080)
        );
        assert_eq!(
            (snapshot.current_width, snapshot.current_height),
            (1920, 1080)
        );
        assert_eq!(
            snapshot
                .modes
                .into_iter()
                .map(|mode| (mode.width, mode.height))
                .collect::<Vec<_>>(),
            vec![(1280, 720), (1600, 900), (1920, 1080)]
        );
        assert_eq!(display.current().unwrap().mode, mode(1920, 1080));
        assert!(matches!(
            ModeSession::open(display.clone(), store.clone(), owner.clone(), "two".into()),
            Err(DisplayError::Busy)
        ));
        assert_eq!(session.apply(1600, 900).unwrap().mode, mode(1600, 900));
        assert_eq!(session.apply(1280, 720).unwrap().mode, mode(1280, 720));
    }
    assert_eq!(display.current().unwrap().mode, mode(1920, 1080));
    assert!(store.0.lock().unwrap().is_none());
    assert!(ModeSession::open(display.clone(), store, owner, "two".into()).is_ok());
}

#[test]
fn failed_apply_rolls_back_without_losing_first_original() {
    let mut display = display();
    let mut session = ModeSession::open(
        display.clone(),
        Store::default(),
        Arc::new(AtomicBool::new(false)),
        "one".into(),
    )
    .unwrap();
    session.apply(1600, 900).unwrap();
    display.0.lock().unwrap().1 = true;
    assert!(matches!(
        session.apply(1280, 720),
        Err(DisplayError::Failed(_))
    ));
    assert_eq!(display.current().unwrap().mode, mode(1600, 900));
    assert_eq!(session.restore().unwrap().mode, mode(1920, 1080));
}

#[test]
fn disconnect_restores_original_after_rollback_confirmation_query_fails() {
    struct RollbackQueryFailure {
        display: Display,
        failed_apply: bool,
        fail_next_query: bool,
    }
    impl Backend for RollbackQueryFailure {
        fn current(&mut self) -> Result<State, DisplayError> {
            if std::mem::take(&mut self.fail_next_query) {
                return Err(DisplayError::Failed("transient query failure".into()));
            }
            self.display.current()
        }
        fn modes(&mut self) -> Result<Vec<Mode>, DisplayError> {
            self.display.modes()
        }
        fn test(&mut self, mode: &Mode) -> Result<(), DisplayError> {
            self.display.test(mode)
        }
        fn apply(&mut self, mode: &Mode) -> Result<(), DisplayError> {
            let result = self.display.apply(mode);
            self.fail_next_query = self.failed_apply && result.is_ok();
            self.failed_apply = result.is_err();
            result
        }
    }
    let mut display = display();
    let backend = RollbackQueryFailure {
        display: display.clone(),
        failed_apply: false,
        fail_next_query: false,
    };
    let mut session = ModeSession::open(
        backend,
        Store::default(),
        Arc::new(AtomicBool::new(false)),
        "one".into(),
    )
    .unwrap();
    session.apply(1600, 900).unwrap();
    display.0.lock().unwrap().1 = true;
    assert_eq!(
        session.apply(1280, 720),
        Err(DisplayError::Failed("transient query failure".into()))
    );
    assert_eq!(display.current().unwrap().mode, mode(1600, 900));
    drop(session);
    assert_eq!(display.current().unwrap().mode, mode(1920, 1080));
}

#[test]
fn local_changes_are_not_overwritten_by_apply_or_disconnect() {
    let mut display = display();
    let store = Store::default();
    let owner = Arc::new(AtomicBool::new(false));
    let mut session =
        ModeSession::open(display.clone(), store.clone(), owner.clone(), "one".into()).unwrap();
    session.apply(1280, 720).unwrap();
    display.0.lock().unwrap().0.mode = mode(1600, 900);
    assert_eq!(
        session.apply(1920, 1080),
        Err(DisplayError::ExternalChanged)
    );
    assert_eq!(session.restore(), Err(DisplayError::ExternalChanged));
    drop(session);
    assert_eq!(display.current().unwrap().mode, mode(1600, 900));
    assert!(store.0.lock().unwrap().is_none());
    assert!(ModeSession::open(display.clone(), store.clone(), owner, "new".into()).is_ok());
    assert_eq!(display.current().unwrap().mode, mode(1600, 900));
    assert!(store.0.lock().unwrap().is_none());
}

#[test]
fn interrupted_apply_is_recovered_before_a_new_session_snapshot() {
    let mut display = display();
    let original = display.current().unwrap();
    display.0.lock().unwrap().0.mode = mode(1280, 720);
    let store = Store::default();
    *store.0.lock().unwrap() = Some(Journal {
        version: 1,
        owner: "dead".into(),
        original,
        previous: mode(1600, 900),
        requested: mode(1280, 720),
    });
    let owner = Arc::new(AtomicBool::new(false));
    display.0.lock().unwrap().0.mode = mode(1024, 768);
    assert!(matches!(
        ModeSession::open(display.clone(), store.clone(), owner.clone(), "new".into()),
        Err(DisplayError::ExternalChanged)
    ));
    assert_eq!(display.current().unwrap().mode, mode(1024, 768));
    assert!(store.0.lock().unwrap().is_some());
    display.0.lock().unwrap().0.mode = mode(1280, 720);
    let mut session =
        ModeSession::open(display.clone(), store.clone(), owner, "new".into()).unwrap();
    assert_eq!(display.current().unwrap().mode, mode(1920, 1080));
    assert!(store.0.lock().unwrap().is_none());
    assert_eq!(session.apply(1111, 777), Err(DisplayError::UnsupportedMode));
    session.apply(1600, 900).unwrap();
    assert_eq!(session.apply(0, 0).unwrap().mode, mode(1920, 1080));
}
