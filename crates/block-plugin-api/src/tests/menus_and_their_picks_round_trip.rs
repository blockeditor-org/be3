use super::*;

fn command(id: &str, label: &str, check: MenuCheck) -> MenuEntry {
    MenuEntry::Command(MenuCommand {
        id: id.into(),
        label: label.into(),
        glyph: String::new(),
        enabled: true,
        check,
        shortcut: None,
        pin: MenuPin::None,
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
            pin: MenuPin::Icon,
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
    assert_eq!(
        MenuEntry::nest(MenuEntry::flatten(&entries)),
        Ok(entries),
        "a menu flattened for the wire nests back as it was"
    );

    let oversized = vec![
        nested(1),
        command(&"x".repeat(MAX_STRING_BYTES + 1), "", MenuCheck::None),
    ];
    assert_eq!(
        MenuEntry::nest(MenuEntry::flatten(&oversized)),
        Err(DecodeError::LimitExceeded("string"))
    );

    let deep = vec![nested(MAX_MENU_DEPTH + 1)];
    assert_eq!(
        MenuEntry::nest(MenuEntry::flatten(&deep)),
        Err(DecodeError::LimitExceeded("menu depth"))
    );
    let skipping = vec![MenuItem {
        depth: 1,
        kind: MenuItemKind::Separator,
    }];
    assert_eq!(
        MenuEntry::nest(skipping),
        Err(DecodeError::MalformedPayload),
        "an entry deeper than any submenu before it is refused"
    );
}
