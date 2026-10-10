use std::time::Duration;

use super::*;

#[test]
fn a_notification_that_arrives_on_an_idle_desktop_keeps_its_full_time() {
    let mut fixture = Fixture::new();
    fixture.test.settle();
    fixture.test.wait(Duration::from_secs(60));
    fixture
        .test
        .set_host_value::<Notifications>(&NotificationInbox {
            requests: vec![(
                1,
                NotificationRequest::Notify(Box::new(notification(
                    1,
                    "Mail",
                    "Mia Chen",
                    "Lunch at noon?",
                ))),
            )],
        });
    fixture.test.wait(Duration::from_secs(1));
    fixture.settle();
    assert!(
        fixture.test.shown(&toast(1)),
        "the toast counts its time from the frame that shows it, not the last one drawn"
    );

    fixture.test.advance(Duration::from_secs(2));
    assert!(fixture.test.shown(&toast(1)), "its time is not up yet");

    fixture.test.advance(Duration::from_secs(4));
    fixture.settle();
    assert!(!fixture.test.shown(&toast(1)), "its time is up");
}
