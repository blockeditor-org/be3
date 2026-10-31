use block_editor_beui::beui::Key;
use block_editor_beui::{HostNotificationAction, NotificationCloseReason};

use super::*;

fn action(key: &str, label: &str) -> HostNotificationAction {
    HostNotificationAction {
        key: key.to_owned(),
        label: label.to_owned(),
    }
}

#[test]
fn a_toast_answers_its_actions_and_hides_while_the_history_is_open() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.notify(&[
        IncomingNotification {
            actions: vec![action(DEFAULT_ACTION, "Open"), action("reply", "Reply")],
            ..notification(1, "Mail", "Mia Chen", "Lunch at noon?")
        },
        IncomingNotification {
            resident: true,
            expire_timeout: 0,
            actions: vec![action("pause", "Pause")],
            ..notification(2, "Player", "Now playing", "")
        },
        notification(3, "Mail", "Sam", "See you there"),
    ]);
    fixture
        .test
        .take_host_calls(block_ui_test::requested::notification_report);

    fixture.test.click("desktop.clock");
    fixture.settle();
    assert!(
        !fixture.test.shown(&toast(1)),
        "the toasts make way for the list of notifications"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert!(
        fixture.test.shown(&toast(1)),
        "and come back once it closes"
    );

    fixture.test.click(format!("{}.action.reply", toast(1)));
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![
            NotificationSignal::ActionInvoked {
                id: 1,
                key: "reply".to_owned()
            },
            NotificationSignal::Closed {
                id: 1,
                reason: NotificationCloseReason::Dismissed
            },
        ]
    );
    assert!(!fixture.test.shown(&toast(1)));

    fixture.test.click(format!("{}.action.pause", toast(2)));
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![NotificationSignal::ActionInvoked {
            id: 2,
            key: "pause".to_owned()
        }],
        "a resident notification stays open"
    );
    assert!(!fixture.test.shown(&toast(2)), "but its toast goes");

    fixture.test.click(format!("{}.dismiss", toast(3)));
    fixture.settle();
    assert_eq!(
        fixture.signals(),
        vec![NotificationSignal::Closed {
            id: 3,
            reason: NotificationCloseReason::Dismissed
        }]
    );

    fixture.test.click("desktop.clock");
    fixture.settle();
    assert!(
        fixture.says("Now playing"),
        "the resident one is still listed"
    );
}
