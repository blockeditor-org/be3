use super::*;

#[test]
fn an_answer_to_an_earlier_attempt_unlocks_nothing() {
    let mut lock = locked();
    let mut recorder = Recorder::default();
    assert!(
        !lock.answered(0, Verdict::Accepted, seconds(0)),
        "an answer nobody asked for is ignored"
    );
    assert!(!lock.answered(1, Verdict::Accepted, seconds(0)));
    assert!(lock.locked());

    lock.submit(password("first"), seconds(1), &mut recorder);
    let first = recorder.last();
    assert!(lock.answered(first, Verdict::Accepted, seconds(2)));
    lock.lock(Trigger::Idle);
    assert!(
        !lock.answered(first, Verdict::Accepted, seconds(3)),
        "an answer from before the screen locked again is ignored"
    );
    assert!(lock.locked());

    lock.submit(password("second"), seconds(4), &mut recorder);
    let second = recorder.last();
    assert!(!lock.answered(second + 1, Verdict::Accepted, seconds(5)));
    assert!(lock.locked());
    assert!(lock.answered(second, Verdict::Accepted, seconds(5)));
}
