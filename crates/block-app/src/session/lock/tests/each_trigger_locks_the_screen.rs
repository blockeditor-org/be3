use super::*;

#[test]
fn each_trigger_locks_the_screen() {
    for trigger in [
        Trigger::Shortcut,
        Trigger::Desktop,
        Trigger::Logind,
        Trigger::Sleep,
    ] {
        let mut lock = Lock::default();
        assert!(!lock.locked());
        assert!(lock.lock(trigger), "{trigger:?} locks");
        assert!(lock.locked());
        assert!(
            !lock.lock(Trigger::Desktop),
            "locking again changes nothing"
        );
        assert_eq!(
            lock.state(seconds(0)),
            LockState {
                locked: true,
                checking: false,
                retry_in: None,
                error: None,
            }
        );
    }
}
