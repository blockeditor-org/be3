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
    let messages = [
        (LinuxMessage::WatchNotifications, Direction::ToHost),
        (
            LinuxMessage::Notifications(vec![notification]),
            Direction::ToPlugin,
        ),
        (
            LinuxMessage::InvokeNotification {
                id: 7,
                action: "reply".into(),
            },
            Direction::ToHost,
        ),
        (
            LinuxMessage::DismissNotifications(vec![7, 8]),
            Direction::ToHost,
        ),
    ];
    for (message, direction) in messages {
        assert_eq!(message.direction(), direction);
        let message = Message::Editor(EditorMessage::Linux {
            instance: EditorInstanceId(4),
            message,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
