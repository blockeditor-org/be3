use super::*;

#[test]
fn requests_wait_for_the_desktop_until_it_says_it_has_them() {
    let (mut inbox, ids, mut sent) = inbox();
    let first = notify(&ids, &mut sent, 0);
    sent.push(NotificationRequest::Close(first));
    let (changed, signals) = inbox.frame(std::mem::take(&mut sent), Vec::new());
    assert!(changed);
    assert!(signals.is_empty());
    assert_eq!(sequences(&inbox), vec![1, 2]);

    let (changed, _) = inbox.frame(std::mem::take(&mut sent), Vec::new());
    assert!(!changed, "nothing new arrived");

    let report = NotificationReport {
        received: Some(1),
        signals: vec![NotificationSignal::ActionInvoked {
            id: first,
            key: "default".to_owned(),
        }],
        kept: vec![first],
    };
    let (changed, signals) = inbox.frame(std::mem::take(&mut sent), vec![report]);
    assert!(changed);
    assert_eq!(
        sequences(&inbox),
        vec![2],
        "only what the desktop has not had is kept"
    );
    assert_eq!(
        signals,
        vec![NotificationSignal::ActionInvoked {
            id: first,
            key: "default".to_owned()
        }],
        "what the desktop reports goes on to the bus"
    );

    for _ in 0..MAX_PENDING + 2 {
        notify(&ids, &mut sent, 0);
    }
    let (_, signals) = inbox.frame(std::mem::take(&mut sent), Vec::new());
    assert_eq!(inbox.published().requests.len(), MAX_PENDING);
    assert_eq!(
        signals.len(),
        2,
        "the oldest notifications nobody took are closed as expired: {signals:?}"
    );
    assert!(signals.iter().all(|signal| matches!(
        signal,
        NotificationSignal::Closed {
            id: _,
            reason: NotificationCloseReason::Expired
        }
    )));
}
