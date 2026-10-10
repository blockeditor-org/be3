use super::*;

#[test]
fn a_password_that_cannot_be_checked_leaves_the_screen_locked() {
    let mut lock = locked();
    let mut recorder = Recorder::default();

    lock.submit(password("hunter2"), seconds(1), &mut recorder);
    lock.answered(
        recorder.last(),
        Verdict::Failed("PAM is not installed".to_owned()),
        seconds(2),
    );
    assert!(lock.locked(), "an error is never an unlock");
    let state = lock.state(seconds(2));
    assert!(!state.busy(), "the field can be tried again");
    assert!(
        state
            .message()
            .is_some_and(|error| error.contains("PAM is not installed")),
        "the problem is shown"
    );

    lock.submit(password("hunter2"), seconds(3), &mut recorder);
    assert_eq!(recorder.attempts.len(), 2);
}
