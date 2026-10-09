use super::*;

#[test]
fn a_new_notification_gets_a_fresh_id_and_a_replacement_keeps_its_own() {
    let mut center = Center::default();
    let first = center.notify(incoming("one"), seconds(0), 10);
    let second = center.notify(incoming("two"), seconds(0), 11);
    assert_ne!(first, 0, "zero means no notification");
    assert_ne!(first, second);
    assert_eq!(listed(&center), vec![second, first], "the newest is first");

    let revision = center.revision();
    let replaced = center.notify(
        Incoming {
            replaces_id: first,
            ..incoming("one, again")
        },
        seconds(1),
        12,
    );
    assert_eq!(replaced, first, "a replacement keeps the id it replaced");
    assert!(center.revision() > revision);
    assert_eq!(listed(&center), vec![second, first], "in its own place");
    let kept = center.listed().find(|kept| kept.id == first).unwrap();
    assert_eq!(kept.incoming.summary, "one, again");
    assert_eq!(kept.received, 12);

    let unknown = center.notify(
        Incoming {
            replaces_id: 999,
            ..incoming("three")
        },
        seconds(1),
        13,
    );
    assert!(
        ![first, second].contains(&unknown),
        "replacing one that is gone makes a new one"
    );
    assert!(center.take_signals().is_empty());
}
