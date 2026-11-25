use super::*;

#[test]
fn a_root_is_shown_why_a_password_was_refused_and_when_to_retry() {
    let mut lock = locked();
    let mut recorder = Recorder::default();
    let now = seconds(10);
    for _ in 0..FREE_ATTEMPTS {
        lock.submit(password("wrong"), now, &mut recorder);
        assert!(lock.checking(), "a password handed over is being checked");
        lock.answered(
            recorder.last(),
            Verdict::Denied("That password is not right.".to_owned()),
            now,
        );
        assert!(!lock.checking(), "the check is over once answered");
    }
    assert_eq!(
        lock.refused(),
        Some("That password is not right."),
        "why it was refused, without the wait, which a root counts down itself"
    );
    assert_eq!(
        lock.retry_at(now),
        Some(now + FIRST_WAIT),
        "the wait ends on the host's clock"
    );
    assert_eq!(
        lock.retry_at(now + FIRST_WAIT),
        None,
        "no wait once it is over"
    );

    // Locked again after it was unlocked, it shows no earlier refusal.
    let mut unlocked = locked();
    unlocked.submit(password("wrong"), now, &mut recorder);
    unlocked.answered(
        recorder.last(),
        Verdict::Denied("That password is not right.".to_owned()),
        now,
    );
    unlocked.submit(password("right"), now, &mut recorder);
    unlocked.answered(recorder.last(), Verdict::Accepted, now);
    assert_eq!(unlocked.refused(), None, "an unlocked session shows no refusal");
}
