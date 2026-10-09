use super::*;

#[test]
fn the_right_password_unlocks_the_screen() {
    let mut lock = locked();
    let mut recorder = Recorder::default();

    assert_eq!(lock.submit(password(""), seconds(1), &mut recorder), None);
    assert!(recorder.attempts.is_empty(), "an empty field is not checked");

    lock.submit(password("hunter2"), seconds(1), &mut recorder);
    assert_eq!(recorder.attempts.len(), 1);
    assert_eq!(recorder.attempts[0].1, "hunter2");
    assert!(lock.state(seconds(1)).busy, "the field waits for the answer");
    lock.submit(password("again"), seconds(1), &mut recorder);
    assert_eq!(recorder.attempts.len(), 1, "one check at a time");
    assert!(lock.locked(), "nothing unlocks before the answer");

    assert!(lock.answered(recorder.last(), Verdict::Accepted, seconds(2)));
    assert!(!lock.locked());
    assert_eq!(lock.state(seconds(2)), LockState::default());
}
