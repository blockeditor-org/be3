use super::*;

#[test]
fn media_messages_round_trip() {
    let levels = MediaLevels {
        output: Some(MediaLevel {
            level: 0.45,
            muted: false,
        }),
        input: Some(MediaLevel {
            level: 1.0,
            muted: true,
        }),
        brightness: Some(0.3),
    };
    let mut messages = vec![
        LinuxMessage::WatchMedia,
        LinuxMessage::Media(levels),
        LinuxMessage::Media(MediaLevels::default()),
        LinuxMessage::RequestMedia(MediaRequest::StepVolume(-0.05)),
        LinuxMessage::RequestMedia(MediaRequest::SetVolume(0.7)),
        LinuxMessage::RequestMedia(MediaRequest::ToggleMute),
        LinuxMessage::RequestMedia(MediaRequest::ToggleMicMute),
        LinuxMessage::RequestMedia(MediaRequest::StepBrightness(0.05)),
    ];
    messages.extend(
        PlayerCommand::ALL.map(|command| LinuxMessage::RequestMedia(MediaRequest::Player(command))),
    );
    for message in messages {
        let to_plugin = matches!(message, LinuxMessage::Media(_));
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
}
