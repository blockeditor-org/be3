use super::*;

#[test]
fn a_toast_hides_when_its_time_is_up_and_the_notification_stays_listed() {
    let mut center = Center::default();
    let usual = center.notify(incoming("usual"), seconds(0), 0);
    let quick = center.notify(
        Incoming {
            expire_timeout: 1500,
            ..incoming("quick")
        },
        seconds(0),
        0,
    );
    let never = center.notify(
        Incoming {
            expire_timeout: 0,
            ..incoming("never")
        },
        seconds(0),
        0,
    );
    let critical = center.notify(
        Incoming {
            urgency: Urgency::Critical,
            ..incoming("critical")
        },
        seconds(0),
        0,
    );
    let transient = center.notify(
        Incoming {
            transient: true,
            ..incoming("transient")
        },
        seconds(0),
        0,
    );
    assert_eq!(
        toasted(&center),
        vec![usual, quick, never, critical, transient]
    );
    assert_eq!(
        listed(&center),
        vec![critical, never, quick, usual],
        "a transient notification is never listed"
    );

    assert_eq!(
        center.frame(seconds(1)),
        Some(Duration::from_millis(500)),
        "the next toast to go is the quick one"
    );
    assert_eq!(center.frame(seconds(2)), Some(seconds(3)));
    assert_eq!(toasted(&center), vec![usual, never, critical, transient]);
    assert!(
        center.take_signals().is_empty(),
        "a hidden toast is not closed"
    );

    assert_eq!(center.frame(DEFAULT_TIMEOUT), None);
    assert_eq!(
        toasted(&center),
        vec![never, critical],
        "a timeout of zero and critical urgency wait to be answered"
    );
    assert_eq!(
        center.take_signals(),
        vec![Signal::Closed(transient, CloseReason::Expired)],
        "a transient notification closes when its time is up"
    );
    assert_eq!(listed(&center), vec![critical, never, quick, usual]);

    center.notify(
        Incoming {
            replaces_id: usual,
            ..incoming("usual, again")
        },
        seconds(10),
        0,
    );
    assert_eq!(
        toasted(&center),
        vec![usual, never, critical],
        "a replacement shows again"
    );
}
