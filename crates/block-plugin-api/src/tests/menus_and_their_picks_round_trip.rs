use super::*;

fn command(id: &str, label: &str, check: MenuCheck) -> MenuEntry {
    MenuEntry::Command(MenuCommand {
        id: id.into(),
        label: label.into(),
        glyph: String::new(),
        enabled: true,
        check,
        shortcut: None,
    })
}

fn nested(depth: usize) -> MenuEntry {
    match depth {
        0 => command("leaf", "Leaf", MenuCheck::None),
        depth => MenuEntry::Submenu(MenuSubmenu {
            id: format!("level.{depth}"),
            label: "Level".into(),
            entries: vec![nested(depth - 1)],
            ..MenuSubmenu::default()
        }),
    }
}

#[test]
fn menus_and_their_picks_round_trip() {
    let entries = vec![
        MenuEntry::Command(MenuCommand {
            id: "editor.undo".into(),
            label: "Undo".into(),
            glyph: "undo".into(),
            enabled: false,
            check: MenuCheck::None,
            shortcut: Some(KeyChord {
                key: Key::Z,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
                tap: false,
            }),
        }),
        MenuEntry::Separator,
        MenuEntry::Submenu(MenuSubmenu {
            id: "canvas.view".into(),
            label: "View".into(),
            glyph: String::new(),
            entries: vec![
                command("canvas.grid", "Show grid", MenuCheck::Checkbox(true)),
                MenuEntry::Separator,
                command("canvas.grid.small", "Small", MenuCheck::Radio(false)),
                command("canvas.grid.medium", "Medium", MenuCheck::Radio(true)),
            ],
        }),
        command("canvas.delete", "Delete", MenuCheck::None),
    ];
    assert_eq!(
        MenuEntry::command(&entries, "canvas.grid.medium").map(|command| command.check),
        Some(MenuCheck::Radio(true)),
        "a command is found inside its submenu"
    );
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
        entries: vec![
            nested(1),
            command(&"x".repeat(MAX_STRING_BYTES + 1), "", MenuCheck::None),
        ],
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );

    let deep = Message::Editor(EditorMessage::Menu {
        instance: EditorInstanceId(3),
        entries: vec![nested(MAX_MENU_DEPTH + 1)],
    });
    assert_eq!(
        encode_frame(&deep),
        Err(DecodeError::LimitExceeded("menu depth"))
    );
}
