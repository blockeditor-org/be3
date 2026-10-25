use super::*;

#[test]
fn audio_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::PlayAudio {
            instance: EditorInstanceId(5),
            command: AudioCommand::Load(crate::AudioSource {
                media_type: "audio/flac".into(),
                bytes: vec![1, 2, 3],
            }),
        }),
        Message::Editor(EditorMessage::PlayAudio {
            instance: EditorInstanceId(5),
            command: AudioCommand::Toggle,
        }),
        Message::Editor(EditorMessage::AudioStatus {
            instance: EditorInstanceId(5),
            status: AudioStatus {
                playing: true,
                position_micros: 1_500_000,
                duration_micros: Some(9_000_000),
                error: None,
            },
        }),
        Message::Editor(EditorMessage::AudioStatus {
            instance: EditorInstanceId(5),
            status: AudioStatus {
                error: Some("no audio output".into()),
                ..AudioStatus::default()
            },
        }),
    ];
    for message in messages {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
