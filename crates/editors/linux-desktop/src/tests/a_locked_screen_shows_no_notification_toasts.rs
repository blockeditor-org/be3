use block_editor_beui::ScreenLocked;

use super::*;

#[test]
fn a_locked_screen_shows_no_notification_toasts() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.notify(&[notification(
        1,
        "Mail",
        "From Ada",
        "The password is hunter2",
    )]);
    assert!(fixture.test.shown(&toast(1)));
    assert!(fixture.says("hunter2"));

    fixture.test.set_host_value::<ScreenLocked>(&true);
    fixture.settle();
    assert!(
        !fixture.test.shown(&toast(1)) && !fixture.says("hunter2"),
        "nothing a notification says shows while the screen is locked"
    );

    fixture.notify(&[notification(2, "Mail", "From Ada again", "")]);
    assert!(
        !fixture.says("From Ada again"),
        "nor does one that arrives while it is"
    );

    fixture.test.set_host_value::<ScreenLocked>(&false);
    fixture.settle();
    assert!(
        fixture.test.shown(&toast(1)) && fixture.test.shown(&toast(2)),
        "the toasts are still there once the screen is unlocked"
    );
}
