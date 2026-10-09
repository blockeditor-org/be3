use std::time::Duration;

use super::*;

#[test]
fn a_locked_screen_shows_no_notification_toasts() {
    let mut center = Center::default();
    center.notify(
        center::Incoming {
            app_name: "Mail".to_owned(),
            summary: "From Ada".to_owned(),
            body: "The password is hunter2".to_owned(),
            expire_timeout: -1,
            ..center::Incoming::default()
        },
        Duration::ZERO,
        0,
    );

    let shown = toasts(&center, false);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].title, "From Ada");

    assert!(
        toasts(&center, true).is_empty(),
        "nothing a notification says shows over the lock screen"
    );
    assert_eq!(
        toasts(&center, false).len(),
        1,
        "the toast is still there once the screen is unlocked"
    );
}
