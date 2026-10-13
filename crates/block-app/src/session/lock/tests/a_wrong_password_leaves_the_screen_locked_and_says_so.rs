use super::*;

#[test]
fn a_wrong_password_leaves_the_screen_locked_and_says_so() {
    let mut lock = locked();
    let mut recorder = Recorder::default();

    lock.submit(password("wrong"), seconds(1), &mut recorder);
    let unlocked = lock.answered(
        recorder.last(),
        Verdict::Denied("That password is not right.".to_owned()),
        seconds(2),
    );
    assert!(!unlocked);
    assert!(lock.locked());
    assert_eq!(
        lock.state(seconds(2)),
        LockState {
            locked: true,
            busy: false,
            error: Some("That password is not right.".to_owned()),
        }
    );

    lock.submit(password("right"), seconds(3), &mut recorder);
    assert_eq!(
        lock.state(seconds(3)).error,
        None,
        "a new attempt clears the old error"
    );
    assert!(lock.answered(recorder.last(), Verdict::Accepted, seconds(4)));
    assert!(!lock.locked());
}
