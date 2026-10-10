use super::*;

#[test]
fn the_lock_screen_sends_the_password_and_shows_the_hosts_answer() {
    let mut test = lock_screen(locked());
    assert!(test.watches::<ScreenLock>());
    assert!(test.label("desktop.lock.0").contains("Ada Lovelace"));
    test.snapshot("the_desktop_lock_screen");

    test.text("hunter2");
    test.key_press(Key::Enter);
    test.run();
    assert_eq!(
        test.take_actions::<UnlockAttempt>(),
        vec![UnlockAttempt {
            password: "hunter2".to_owned()
        }],
        "the host is asked to check what was typed"
    );

    test.set_host_value::<ScreenLock>(&LockState {
        checking: true,
        ..locked()
    });
    test.run();
    test.text("again");
    test.key_press(Key::Enter);
    test.run();
    assert!(
        test.take_actions::<UnlockAttempt>().is_empty(),
        "nothing more is sent while the host is checking"
    );

    test.set_host_value::<ScreenLock>(&LockState {
        error: Some("That password is not right.".to_owned()),
        ..locked()
    });
    test.run();
    assert_eq!(
        test.label("desktop.lock.0.error"),
        "That password is not right."
    );
    test.snapshot("the_desktop_lock_screen_after_a_wrong_password");

    test.set_host_value::<ScreenLock>(&LockState {
        retry_in_seconds: Some(4),
        error: Some("That password is not right.".to_owned()),
        ..locked()
    });
    test.run();
    assert_eq!(
        test.label("desktop.lock.0.error"),
        "Too many attempts. Try again in 4 seconds."
    );
    test.text("hunter2");
    test.key_press(Key::Enter);
    test.run();
    assert!(
        test.take_actions::<UnlockAttempt>().is_empty(),
        "nothing is sent while the host makes the next attempt wait"
    );
}
