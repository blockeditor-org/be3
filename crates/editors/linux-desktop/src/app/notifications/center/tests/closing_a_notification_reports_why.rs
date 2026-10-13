use super::*;

#[test]
fn closing_a_notification_reports_why() {
    let mut center = Center::default();
    let (asked, dismissed) = (1, 2);
    center.notify(asked, incoming("asked"), seconds(0), 0);
    center.notify(dismissed, incoming("dismissed"), seconds(0), 0);
    assert!(center.close(asked, NotificationCloseReason::Closed));
    assert!(center.dismiss(dismissed));
    assert!(
        !center.close(asked, NotificationCloseReason::Closed),
        "it is gone"
    );
    assert_eq!(
        center.take_signals(),
        vec![
            NotificationSignal::Closed(asked, NotificationCloseReason::Closed),
            NotificationSignal::Closed(dismissed, NotificationCloseReason::Dismissed),
        ]
    );
    assert!(listed(&center).is_empty());
    assert!(toasted(&center).is_empty());
    assert!(center.kept_ids().is_empty());
    assert_eq!(NotificationCloseReason::Expired as u32, 1);
    assert_eq!(NotificationCloseReason::Dismissed as u32, 2);
    assert_eq!(NotificationCloseReason::Closed as u32, 3);
}
