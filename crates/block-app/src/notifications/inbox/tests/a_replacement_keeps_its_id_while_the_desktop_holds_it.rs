use super::*;

#[test]
fn a_replacement_keeps_its_id_while_the_desktop_holds_it() {
    let (mut inbox, ids, sender, receiver) = inbox();
    let first = notify(&ids, &sender, 0);
    let second = notify(&ids, &sender, 0);
    assert_ne!(first, 0, "zero means no notification");
    assert_ne!(first, second);
    assert_eq!(
        notify(&ids, &sender, first),
        first,
        "a replacement is answered at once with the id it replaced"
    );
    assert_ne!(
        notify(&ids, &sender, 999),
        999,
        "replacing one that was never handed out makes a new one"
    );

    inbox.frame(
        &receiver,
        vec![NotificationReport {
            received: Some(4),
            signals: Vec::new(),
            kept: vec![second],
        }],
    );
    let fresh = notify(&ids, &sender, first);
    assert_ne!(
        fresh, first,
        "once the desktop has let one go, replacing it makes a new one"
    );
    assert_eq!(
        notify(&ids, &sender, second),
        second,
        "one the desktop still holds is replaced"
    );
}
