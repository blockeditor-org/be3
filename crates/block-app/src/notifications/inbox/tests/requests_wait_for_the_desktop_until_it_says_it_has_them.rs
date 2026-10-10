use super::*;

#[test]
fn requests_wait_for_the_desktop_until_it_says_it_has_them() {
    let (mut inbox, ids, sender, receiver) = inbox();
    let first = notify(&ids, &sender, 0);
    sender.send(NotificationRequest::Close(first)).unwrap();
    let (changed, signals) = inbox.frame(&receiver, Vec::new());
    assert!(changed);
    assert!(signals.is_empty());
    assert_eq!(sequences(&inbox), vec![1, 2]);

    let (changed, _) = inbox.frame(&receiver, Vec::new());
    assert!(!changed, "nothing new arrived");

    let report = NotificationReport {
        received: Some(1),
        signals: vec![NotificationSignal::ActionInvoked(
            first,
            "default".to_owned(),
        )],
        kept: vec![first],
    };
    let (changed, signals) = inbox.frame(&receiver, vec![report]);
    assert!(changed);
    assert_eq!(
        sequences(&inbox),
        vec![2],
        "only what the desktop has not had is kept"
    );
    assert_eq!(
        signals,
        vec![NotificationSignal::ActionInvoked(
            first,
            "default".to_owned()
        )],
        "what the desktop reports goes on to the bus"
    );

    for _ in 0..MAX_PENDING + 2 {
        notify(&ids, &sender, 0);
    }
    let (_, signals) = inbox.frame(&receiver, Vec::new());
    assert_eq!(inbox.published().requests.len(), MAX_PENDING);
    assert_eq!(
        signals.len(),
        2,
        "the oldest notifications nobody took are closed as expired: {signals:?}"
    );
    assert!(signals.iter().all(|signal| matches!(
        signal,
        NotificationSignal::Closed(_, NotificationCloseReason::Expired)
    )));
}
