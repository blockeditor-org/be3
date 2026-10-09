use super::*;

#[test]
fn power_messages_round_trip() {
    let available = PowerAvailability {
        lock: true,
        suspend: true,
        restart: true,
        power_off: false,
        log_out: true,
    };
    let mut messages = vec![LinuxMessage::WatchPower, LinuxMessage::Power(available)];
    messages.extend(PowerAction::ALL.map(LinuxMessage::RequestPower));
    for message in messages {
        let to_plugin = matches!(message, LinuxMessage::Power(_));
        assert_eq!(
            message.direction(),
            match to_plugin {
                true => Direction::ToPlugin,
                false => Direction::ToHost,
            }
        );
        let message = Message::Editor(EditorMessage::Linux {
            instance: EditorInstanceId(4),
            message,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
    assert!(available.allows(PowerAction::Restart));
    assert!(!available.allows(PowerAction::PowerOff));
}
