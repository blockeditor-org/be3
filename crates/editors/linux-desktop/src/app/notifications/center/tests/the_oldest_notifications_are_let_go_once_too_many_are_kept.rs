use super::*;

#[test]
fn the_oldest_notifications_are_let_go_once_too_many_are_kept() {
    let mut center = Center::default();
    center.notify(1, incoming("first"), seconds(0), 0);
    for index in 0..MAX_KEPT {
        let id = u32::try_from(index).unwrap() + 2;
        center.notify(id, incoming(&format!("{index}")), seconds(0), 0);
    }
    assert_eq!(center.listed().count(), MAX_KEPT);
    assert_eq!(
        center.take_signals(),
        vec![NotificationSignal::Closed(
            1,
            NotificationCloseReason::Expired
        )]
    );
}
