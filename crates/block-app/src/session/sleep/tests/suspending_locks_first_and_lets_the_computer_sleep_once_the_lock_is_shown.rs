use super::*;

#[test]
fn suspending_locks_first_and_lets_the_computer_sleep_once_the_lock_is_shown() {
    let mut guard = SleepGuard::default();
    let mut lock = Lock::default();
    let mut inhibitor = Recorder::default();

    guard.hold(&mut inhibitor);
    guard.hold(&mut inhibitor);
    assert_eq!(inhibitor.taken, 1, "the session holds one delay inhibitor");
    guard.shown(&mut inhibitor);
    assert_eq!(inhibitor.released, 0, "showing the lock alone releases nothing");

    guard.prepare(true, &mut lock, &mut inhibitor);
    assert!(lock.locked(), "the screen locks before the computer sleeps");
    assert!(guard.waiting());
    assert_eq!(
        inhibitor.released, 0,
        "the computer waits until the lock screen is on the screens"
    );
    guard.shown(&mut inhibitor);
    assert_eq!(inhibitor.released, 1, "then it may sleep");
    guard.shown(&mut inhibitor);
    assert_eq!(inhibitor.released, 1);

    guard.prepare(false, &mut lock, &mut inhibitor);
    assert_eq!(inhibitor.taken, 2, "waking takes the inhibitor again");
    assert!(lock.locked(), "and the screen stays locked");

    guard.prepare(true, &mut lock, &mut inhibitor);
    guard.prepare(false, &mut lock, &mut inhibitor);
    assert_eq!(
        (inhibitor.taken, inhibitor.released),
        (2, 1),
        "a suspend that is called off before the lock showed keeps the inhibitor"
    );
}
