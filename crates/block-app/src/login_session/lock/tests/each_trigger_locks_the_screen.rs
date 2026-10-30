use super::*;

#[test]
fn each_trigger_locks_the_screen() {
    for trigger in [
        Trigger::Shortcut,
        Trigger::Menu,
        Trigger::Logind,
        Trigger::Sleep,
        Trigger::Idle,
    ] {
        let mut lock = Lock::default();
        assert!(!lock.locked());
        assert!(lock.lock(trigger), "{trigger:?} locks");
        assert!(lock.locked());
        assert!(
            !lock.lock(Trigger::Shortcut),
            "locking again changes nothing"
        );
        assert_eq!(
            lock.state(seconds(0)),
            LockState {
                locked: true,
                busy: false,
                error: None,
            }
        );
    }
}
