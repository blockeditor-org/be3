use block_editor_beui::{
    HostNotification, HostNotificationAction, NotificationAction, Notifications,
};

use super::*;

const NOON: u64 = 1_768_478_400;

fn notification(id: u32, app_name: &str, summary: &str, body: &str) -> HostNotification {
    HostNotification {
        id,
        app_name: app_name.to_owned(),
        summary: summary.to_owned(),
        body: body.to_owned(),
        received: NOON + u64::from(id) * 60,
        critical: false,
        actions: Vec::new(),
    }
}

#[test]
fn the_notifications_button_lists_what_arrived_and_answers_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.click("desktop.notifications");
    fixture.settle();
    assert!(fixture.says("Nothing new."), "the list starts empty");

    let message = HostNotification {
        actions: vec![HostNotificationAction {
            key: HostNotification::DEFAULT_ACTION.to_owned(),
            label: "Open".to_owned(),
        }],
        ..notification(
            1,
            "Mail",
            "Mia Chen",
            "Lunch at noon? The new place on Bridge Street has a table free.",
        )
    };
    let update = notification(2, "Software", "Updates are ready", "");
    fixture
        .test
        .set_host_value::<Notifications>(&vec![update.clone(), message.clone()]);
    fixture.settle();
    fixture.test.settle();
    assert!(fixture.says("Updates are ready"));
    assert!(fixture.says("Mia Chen"));
    fixture.test.snapshot("the_notification_history");

    fixture.test.click("desktop.notifications.2.dismiss");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<NotificationAction>(),
        vec![NotificationAction::Dismiss(vec![2])]
    );

    fixture.test.click("desktop.notifications.2");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<NotificationAction>(),
        vec![NotificationAction::Dismiss(vec![2])],
        "one with nothing to open is dismissed by a click"
    );

    fixture.test.click("desktop.notifications.1");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<NotificationAction>(),
        vec![NotificationAction::Invoke {
            id: 1,
            action: HostNotification::DEFAULT_ACTION.to_owned(),
        }],
        "one with a default action is opened by a click"
    );
    assert!(
        !fixture.test.shown("desktop.notifications.list"),
        "and the list closes"
    );

    fixture.test.click("desktop.notifications");
    fixture.settle();
    fixture.test.click("desktop.notifications.clear");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<NotificationAction>(),
        vec![NotificationAction::Dismiss(vec![2, 1])]
    );
}
