use super::*;

#[test]
fn a_lock_the_browser_drops_while_asked_for_is_reported_once() {
    let mut lock = Lock::default();
    lock.want(true);
    assert_eq!(lock.changed(true), None);
    assert!(lock.held());
    assert_eq!(
        lock.changed(false),
        Some(Input::PointerUnlocked),
        "Escape or a blur took the lock while the app still asks, so the app hears it is gone"
    );
    assert!(!lock.held(), "and motion stops being read as a held pointer's");
    assert_eq!(
        lock.refused(),
        None,
        "a second sign of the same loss is not reported again"
    );
}
