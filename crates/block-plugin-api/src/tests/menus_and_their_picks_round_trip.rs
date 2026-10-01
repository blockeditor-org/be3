use super::*;

#[test]
fn menus_and_their_picks_round_trip() {
    let entries = vec![
        MenuEntry {
            id: "editor.undo".into(),
            label: "Undo".into(),
            glyph: "undo".into(),
            enabled: false,
        },
        MenuEntry {
            id: "canvas.delete".into(),
            label: "Delete".into(),
            glyph: String::new(),
            enabled: true,
        },
    ];
    for message in [
        Message::Editor(EditorMessage::Menu {
            instance: EditorInstanceId(3),
            entries: entries.clone(),
        }),
        Message::Editor(EditorMessage::MenuPick {
            instance: EditorInstanceId(3),
            id: "canvas.delete".into(),
        }),
        Message::Editor(EditorMessage::ChildMenuPick {
            instance: EditorInstanceId(2),
            child: ChildId(7),
            id: "editor.undo".into(),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }

    let oversized = Message::Editor(EditorMessage::Menu {
        instance: EditorInstanceId(3),
        entries: vec![MenuEntry {
            label: "x".repeat(MAX_STRING_BYTES + 1),
            ..MenuEntry::default()
        }],
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
