use super::*;

use block_plugin_api::{HostNotification, LinuxMessage};

fn notifications_sent(messages: &[Message]) -> Vec<Vec<HostNotification>> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Linux {
                instance,
                message: LinuxMessage::Notifications(notifications),
            }) if *instance == INSTANCE => Some(notifications.clone()),
            _ => None,
        })
        .collect()
}

fn notification(id: u32) -> HostNotification {
    HostNotification {
        id,
        app_name: "Mail".into(),
        summary: format!("Message {id}"),
        body: String::new(),
        received: 0,
        critical: false,
        actions: Vec::new(),
    }
}

#[test]
fn an_instance_watching_notifications_is_told_of_them_and_its_requests_reach_the_host() {
    let mut instances = placed();
    instances.next_screens(PASS);

    let first = Arc::new(vec![notification(1)]);
    assert!(
        !instances.set_notifications(Arc::clone(&first)),
        "nobody is watching yet"
    );
    assert!(notifications_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.editor_message(EditorMessage::Linux {
        instance: INSTANCE,
        message: LinuxMessage::WatchNotifications,
    }));
    assert_eq!(
        notifications_sent(&instances.next_screens(PASS).opened),
        vec![first.to_vec()]
    );
    assert!(notifications_sent(&instances.next_screens(PASS).opened).is_empty());

    let second = Arc::new(vec![notification(2), notification(1)]);
    assert!(instances.set_notifications(Arc::clone(&second)));
    assert_eq!(
        notifications_sent(&instances.next_screens(PASS).opened),
        vec![second.to_vec()]
    );

    let invoke = LinuxMessage::InvokeNotification {
        id: 2,
        action: "default".into(),
    };
    let dismiss = LinuxMessage::DismissNotifications(vec![1]);
    for message in [invoke.clone(), dismiss.clone()] {
        assert!(instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message,
        }));
    }
    assert_eq!(
        instances.take_notification_requests(INSTANCE),
        vec![invoke, dismiss]
    );
    assert!(instances.take_notification_requests(INSTANCE).is_empty());
    assert!(
        !instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message: LinuxMessage::Notifications(second.to_vec()),
        }),
        "only the host says what was received"
    );
}
