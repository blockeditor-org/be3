use super::*;

use block_plugin_api::{LinuxMessage, PowerAction, PowerAvailability};

fn power_sent(messages: &[Message]) -> Vec<PowerAvailability> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Linux {
                instance,
                message: LinuxMessage::Power(power),
            }) if *instance == INSTANCE => Some(*power),
            _ => None,
        })
        .collect()
}

#[test]
fn an_instance_watching_power_is_told_what_it_may_do_and_its_requests_reach_the_host() {
    let power = PowerAvailability {
        suspend: true,
        restart: false,
        power_off: true,
        log_out: true,
    };
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(!instances.set_power(power), "nobody is watching yet");
    assert!(power_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.editor_message(EditorMessage::Linux {
        instance: INSTANCE,
        message: LinuxMessage::WatchPower,
    }));
    assert_eq!(
        power_sent(&instances.next_screens(PASS).opened),
        vec![power]
    );
    assert!(power_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(
        !instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message: LinuxMessage::RequestPower(PowerAction::Restart),
        }),
        "what the host does not allow is refused"
    );
    assert_eq!(instances.take_power_request(INSTANCE), None);
    for _ in 0..3 {
        assert!(instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message: LinuxMessage::RequestPower(PowerAction::PowerOff),
        }));
    }
    assert_eq!(
        instances.take_power_request(INSTANCE),
        Some(PowerAction::PowerOff),
        "repeated requests are held as one"
    );
    assert_eq!(instances.take_power_request(INSTANCE), None);
    instances.set_power(PowerAvailability::default());
    assert!(
        !instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message: LinuxMessage::RequestPower(PowerAction::PowerOff),
        }),
        "a host with no power actions, such as a windowed desktop, refuses every request"
    );
    assert!(
        !instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message: LinuxMessage::Power(power),
        }),
        "only the host says what is possible"
    );
}
