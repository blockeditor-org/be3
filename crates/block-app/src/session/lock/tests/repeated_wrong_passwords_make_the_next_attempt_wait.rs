use super::*;

#[test]
fn repeated_wrong_passwords_make_the_next_attempt_wait() {
    let mut lock = locked();
    let mut recorder = Recorder::default();
    let mut now = seconds(10);
    let deny = |lock: &mut Lock, recorder: &mut Recorder, now: Duration| {
        assert_eq!(lock.submit(password("wrong"), now, recorder), None);
        lock.answered(
            recorder.last(),
            Verdict::Denied("That password is not right.".to_owned()),
            now,
        );
    };

    for _ in 0..FREE_ATTEMPTS {
        deny(&mut lock, &mut recorder, now);
        now += seconds(1);
    }
    assert_eq!(recorder.attempts.len(), FREE_ATTEMPTS as usize);
    let state = lock.state(now);
    assert!(state.busy(), "the field waits out the delay");
    assert_eq!(
        state.message().as_deref(),
        Some("Too many attempts. Try again in 4 seconds.")
    );
    assert_eq!(
        lock.submit(password("right"), now, &mut recorder),
        Some(seconds(4)),
        "an attempt during the delay is not checked"
    );
    assert_eq!(recorder.attempts.len(), FREE_ATTEMPTS as usize);

    now += seconds(4);
    assert!(!lock.state(now).busy());
    deny(&mut lock, &mut recorder, now);
    assert_eq!(
        lock.wait(now),
        Some(FIRST_WAIT * 2),
        "each failure past the free ones doubles the delay"
    );
    for _ in 0..8 {
        now += LONGEST_WAIT;
        deny(&mut lock, &mut recorder, now);
    }
    assert_eq!(lock.wait(now), Some(LONGEST_WAIT), "up to a limit");

    now += LONGEST_WAIT;
    lock.submit(password("right"), now, &mut recorder);
    assert!(lock.answered(recorder.last(), Verdict::Accepted, now));
    lock.lock(Trigger::Logind);
    deny(&mut lock, &mut recorder, now);
    assert_eq!(lock.wait(now), None, "unlocking forgives the failures");
}
