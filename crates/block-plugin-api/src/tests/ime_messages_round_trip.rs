use super::*;

#[test]
fn ime_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Ime {
            instance: EditorInstanceId(6),
            region: EditorRegion::Frame,
            area: Some(ImeArea {
                rect: ChildRect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
                cursor: ChildRect {
                    x: 5.0,
                    y: 6.0,
                    width: 1.0,
                    height: 12.0,
                },
                text: Some(ImeText {
                    start: 4,
                    text: "say helo".into(),
                    selection: (8, 8),
                    composing: Some((4, 8)),
                }),
            }),
        }),
        Message::Editor(EditorMessage::Ime {
            instance: EditorInstanceId(6),
            region: EditorRegion::Frame,
            area: None,
        }),
        Message::Input(InputBatch {
            screen: ScreenId(2),
            events: vec![
                InputEvent::Ime(ImeInput::Enabled),
                InputEvent::Ime(ImeInput::SetComposingText("か".into())),
                InputEvent::Ime(ImeInput::CommitText("漢".into())),
                InputEvent::Ime(ImeInput::FinishComposing),
                InputEvent::Ime(ImeInput::SetComposingRegion { start: 4, end: 8 }),
                InputEvent::Ime(ImeInput::ReplaceText {
                    start: 4,
                    end: 8,
                    text: "hello".into(),
                }),
                InputEvent::Ime(ImeInput::DeleteSurrounding {
                    before: 1,
                    after: 2,
                }),
                InputEvent::Ime(ImeInput::SetSelection {
                    anchor: 3,
                    focus: 3,
                }),
                InputEvent::Ime(ImeInput::Disabled),
            ],
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
