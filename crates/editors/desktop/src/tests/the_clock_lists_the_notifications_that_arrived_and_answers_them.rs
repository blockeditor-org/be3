use block_editor_beui::{HostNotificationAction, NotificationCloseReason, NotificationSignal};

use super::*;

#[test]
fn the_clock_lists_the_notifications_that_arrived_and_answers_them() {
    let mut fixture = Fixture::new();
    fixture
        .test
        .set_size(block_editor_beui::beui::Vec2::new(1280.0, 800.0));
    fixture.settle();
    assert!(!fixture.test.shown("desktop.clock.unread"));
    fixture.test.click("desktop.clock");
    fixture.settle();
    assert!(fixture.says("Nothing new."), "the list starts empty");

    let message = IncomingNotification {
        actions: vec![HostNotificationAction {
            key: DEFAULT_ACTION.to_owned(),
            label: "Open".to_owned(),
        }],
        ..notification(
            1,
            "Mail",
            "Mia Chen",
            "Lunch at <b>noon</b>? The new place on Bridge Street has a table free.",
        )
    };
    let update = notification(2, "Software", "Updates are ready", "");
    fixture.notify(&[message, update]);
    fixture.test.settle();
    assert!(fixture.says("Updates are ready"));
    assert!(fixture.says("Lunch at noon?"), "the body loses its markup");
    let reports = fixture
        .test
        .take_host_calls(block_ui_test::requested::notification_report);
    assert_eq!(
        reports
            .last()
            .map(|report| (report.received, report.kept.clone())),
        Some((Some(2), vec![1, 2])),
        "the desktop tells the host what it took and what it holds: {reports:?}"
    );
    assert_eq!(
        fixture.test.label("desktop.clock.unread"),
        "2",
        "the clock counts what is listed"
    );
    fixture.test.snapshot("the_notification_history");

    fixture.test.click("desktop.notifications.2.dismiss");
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![NotificationSignal::Closed {
            id: 2,
            reason: NotificationCloseReason::Dismissed
        }]
    );

    fixture.test.click("desktop.notifications.1");
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![
            NotificationSignal::ActionInvoked {
                id: 1,
                key: DEFAULT_ACTION.to_owned()
            },
            NotificationSignal::Closed {
                id: 1,
                reason: NotificationCloseReason::Dismissed
            },
        ],
        "one with a default action is opened by a click"
    );
    assert!(
        !fixture.test.shown("desktop.notifications.list"),
        "and the list closes"
    );

    fixture.notify(&[
        notification(3, "Mail", "Third", ""),
        notification(4, "Mail", "Fourth", ""),
    ]);
    fixture
        .test
        .take_host_calls(block_ui_test::requested::notification_report);
    fixture.test.click("desktop.clock");
    fixture.settle();
    fixture.test.click("desktop.notifications.3");
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![NotificationSignal::Closed {
            id: 3,
            reason: NotificationCloseReason::Dismissed
        }],
        "one with nothing to open is dismissed by a click"
    );
    fixture.test.click("desktop.notifications.clear");
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![NotificationSignal::Closed {
            id: 4,
            reason: NotificationCloseReason::Dismissed
        }]
    );
    assert!(fixture.says("Nothing new."));
}
