use block_editor_beui::HostNotificationAction;

use super::*;

#[test]
fn a_notification_the_desktop_holds_is_replaced_in_place() {
    let mut center = Center::default();
    let arrived = IncomingNotification {
        id: 1,
        app_name: "Mail".to_owned(),
        summary: "one".to_owned(),
        body: "<b>Mia</b> wrote &amp; asked".to_owned(),
        actions: vec![HostNotificationAction {
            key: DEFAULT_ACTION.to_owned(),
            label: "Open".to_owned(),
        }],
        image: Some(HostImage {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        }),
        expire_timeout: -1,
        ..IncomingNotification::default()
    };
    let first = Incoming::from_host(&arrived);
    assert_eq!(
        first.body, "Mia wrote & asked",
        "the body is shown as plain text"
    );
    assert!(first.image.is_some());
    assert!(
        Incoming::from_host(&IncomingNotification {
            image: Some(HostImage {
                width: 2,
                height: 2,
                rgba: vec![0; 3],
            }),
            ..arrived.clone()
        })
        .image
        .is_none(),
        "a picture whose pixels do not fill it is left out"
    );

    center.notify(1, first, seconds(0), 10);
    center.notify(2, incoming("two"), seconds(0), 11);
    assert_eq!(listed(&center), vec![2, 1], "the newest is first");
    assert!(center.toasts().next().unwrap().has_default_action());

    let revision = center.revision();
    center.notify(1, incoming("one, again"), seconds(1), 12);
    assert!(center.revision() > revision);
    assert_eq!(listed(&center), vec![2, 1], "in its own place");
    let kept = center.listed().find(|kept| kept.id == 1).unwrap();
    assert_eq!(kept.incoming.summary, "one, again");
    assert_eq!(kept.received, 12);
    assert_eq!(center.kept_ids(), vec![1, 2]);
    assert!(center.take_signals().is_empty());
}
