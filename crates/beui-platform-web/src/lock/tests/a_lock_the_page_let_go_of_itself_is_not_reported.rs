use super::*;

#[test]
fn a_lock_the_page_let_go_of_itself_is_not_reported() {
    let mut lock = Lock::default();
    lock.want(true);
    lock.changed(true);
    lock.want(false);
    assert_eq!(
        lock.changed(false),
        None,
        "the app stopped asking, so the lock ending is no news to it"
    );
    assert!(
        !lock.press(PointerButton::Primary),
        "and a press is the app's own"
    );
}
