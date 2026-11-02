use super::*;

#[test]
fn notification_messages_round_trip() {
    let incoming = IncomingNotification {
        id: 7,
        app_name: "Mail".into(),
        summary: "New message".into(),
        body: "Lunch at <b>noon</b>?".into(),
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
        urgency: NotificationUrgency::Critical,
        image: Some(HostImage {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        }),
        transient: false,
        resident: true,
        expire_timeout: -1,
        received: 1_700_000_000,
    };
    watched_round_trips::<NotificationsWatch>(NotificationInbox {
        requests: vec![
            (1, NotificationRequest::Notify(incoming)),
            (2, NotificationRequest::Close(7)),
        ],
    });
    watched_round_trips::<ScreenLockedWatch>(true);
    round_trips(&NotificationReport {
        received: Some(2),
        signals: vec![
            NotificationSignal::ActionInvoked {
                id: 7,
                key: "reply".into(),
            },
            NotificationSignal::Closed {
                id: 7,
                reason: NotificationCloseReason::Dismissed,
            },
        ],
        kept: vec![3, 5],
    });
}
