use super::*;

#[test]
fn lock_screen_messages_round_trip() {
    host_value_round_trips::<ScreenLock>(LockState {
        locked: true,
        user: "Ada Lovelace".into(),
        checking: false,
        retry_in_seconds: Some(4),
        error: Some("That password is not right.".into()),
    });
    host_value_round_trips::<Idle>(IdleState { lock_due: true });
    host_action_round_trips(UnlockAttempt {
        password: "hunter2".into(),
    });
    assert_eq!(
        format!(
            "{:?}",
            UnlockAttempt {
                password: "hunter2".into()
            }
        ),
        "UnlockAttempt(..)",
        "a password is never printed"
    );
}
