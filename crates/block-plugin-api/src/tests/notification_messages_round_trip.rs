use super::*;

#[test]
fn notification_messages_round_trip() {
    let notification = HostNotification {
        id: 7,
        app_name: "Mail".into(),
        summary: "New message".into(),
        body: "Lunch at noon?".into(),
        received: 1_700_000_000,
        critical: false,
        actions: vec![
            HostNotificationAction {
                key: "default".into(),
                label: "Open".into(),
            },
            HostNotificationAction {
                key: "reply".into(),
                label: "Reply".into(),
            },
        ],
    };
    assert!(notification.has_default_action());
    host_value_round_trips::<Notifications>(vec![notification]);
    host_action_round_trips(NotificationAction::Invoke {
        id: 7,
        action: "reply".into(),
    });
    host_action_round_trips(NotificationAction::Dismiss(vec![7, 8]));
}
