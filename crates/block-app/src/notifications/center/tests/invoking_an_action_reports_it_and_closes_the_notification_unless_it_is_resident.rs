use super::*;

#[test]
fn invoking_an_action_reports_it_and_closes_the_notification_unless_it_is_resident() {
    let mut center = Center::default();
    let message = center.notify(
        with_actions("message", &[DEFAULT_ACTION, "reply"]),
        seconds(0),
        0,
    );
    let player = center.notify(
        Incoming {
            resident: true,
            ..with_actions("player", &["pause"])
        },
        seconds(0),
        0,
    );
    assert!(center.toasts().next().unwrap().has_default_action());

    assert!(!center.invoke(message, "forward"), "it has no such action");
    assert!(center.invoke(message, "reply"));
    assert!(!center.invoke(message, DEFAULT_ACTION), "it is gone");
    assert!(center.invoke(player, "pause"));
    assert_eq!(
        center.take_signals(),
        vec![
            Signal::ActionInvoked(message, "reply".to_owned()),
            Signal::Closed(message, CloseReason::Dismissed),
            Signal::ActionInvoked(player, "pause".to_owned()),
        ]
    );
    assert_eq!(listed(&center), vec![player], "a resident one stays listed");
    assert!(toasted(&center).is_empty(), "but its toast goes");
}
