use super::*;

#[test]
fn window_requests_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Window {
            instance: EditorInstanceId(7),
            region: EditorRegion::Frame,
            window: WindowRequests {
                cursor: CursorIcon::Crosshair,
                ime: None,
                handles_back: true,
                wants_keyboard: true,
                intercepted_keys: vec![KeyChord {
                    key: Key::Escape,
                    modifiers: Modifiers::default(),
                    tap: true,
                }],
                pastes: vec![PasteKind::Image, PasteKind::Text],
            },
        }),
        Message::Editor(EditorMessage::ShowKeyboard {
            instance: EditorInstanceId(7),
            region: EditorRegion::Frame,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
