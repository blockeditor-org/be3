use super::*;

#[test]
fn a_refused_lock_is_reported_like_a_dropped_one() {
    let mut lock = Lock::default();
    lock.want(true);
    assert_eq!(
        lock.refused(),
        Some(Input::PointerUnlocked),
        "a lock asked for without a user gesture is refused, and the app hears it does not hold the pointer"
    );
}
