use super::*;

#[test]
fn the_next_press_takes_a_lock_still_asked_for_back_and_is_swallowed() {
    let mut lock = Lock::default();
    lock.want(true);
    lock.changed(true);
    lock.changed(false);
    assert!(
        lock.press(PointerButton::Primary),
        "the app still asks after hearing the lock went, so the next press asks the browser again and reaches nothing"
    );
    assert!(
        !lock.press(PointerButton::Secondary),
        "while that request is pending, other presses are the app's"
    );
    assert!(
        lock.release(PointerButton::Primary),
        "the swallowed press's release is swallowed too"
    );
    assert!(!lock.release(PointerButton::Secondary));
    assert_eq!(lock.changed(true), None);
    assert!(lock.held());
}
