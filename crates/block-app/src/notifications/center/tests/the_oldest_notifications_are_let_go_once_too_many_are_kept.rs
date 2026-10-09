use super::*;

#[test]
fn the_oldest_notifications_are_let_go_once_too_many_are_kept() {
    let mut center = Center::default();
    let first = center.notify(incoming("first"), seconds(0), 0);
    for index in 0..MAX_KEPT {
        center.notify(incoming(&format!("{index}")), seconds(0), 0);
    }
    assert_eq!(center.listed().count(), MAX_KEPT);
    assert_eq!(
        center.take_signals(),
        vec![Signal::Closed(first, CloseReason::Expired)]
    );
}
