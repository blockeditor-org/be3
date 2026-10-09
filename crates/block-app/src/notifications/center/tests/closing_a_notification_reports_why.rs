use super::*;

#[test]
fn closing_a_notification_reports_why() {
    let mut center = Center::default();
    let asked = center.notify(incoming("asked"), seconds(0), 0);
    let dismissed = center.notify(incoming("dismissed"), seconds(0), 0);
    assert!(center.close(asked, CloseReason::Closed));
    assert!(center.dismiss(dismissed));
    assert!(!center.close(asked, CloseReason::Closed), "it is gone");
    assert_eq!(
        center.take_signals(),
        vec![
            Signal::Closed(asked, CloseReason::Closed),
            Signal::Closed(dismissed, CloseReason::Dismissed),
        ]
    );
    assert!(listed(&center).is_empty());
    assert!(toasted(&center).is_empty());
    assert_eq!(CloseReason::Expired as u32, 1);
    assert_eq!(CloseReason::Dismissed as u32, 2);
    assert_eq!(CloseReason::Closed as u32, 3);
}
