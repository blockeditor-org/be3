use super::*;

fn deepened(empty: &[u8], one: &[u8], levels: usize) -> Vec<u8> {
    let count = 8;
    let prefix = &empty[..empty.len() - count];
    let level = &one[prefix.len()..one.len() - count];
    let mut bytes = prefix.to_vec();
    for _ in 0..levels {
        bytes.extend_from_slice(level);
    }
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes
}

fn menu_frame(levels: usize) -> Vec<u8> {
    let message = |entries| {
        let frame = encode_frame(&Message::Editor(EditorMessage::Menu {
            instance: EditorInstanceId(3),
            entries,
        }))
        .unwrap();
        frame[4..].to_vec()
    };
    let empty = message(Vec::new());
    let one = message(vec![MenuEntry::Submenu(MenuSubmenu::default())]);
    let payload = deepened(&empty, &one, levels);
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&payload);
    frame
}

fn tray_value(levels: usize) -> Vec<u8> {
    let value = |menu| {
        encode_host(&vec![HostTrayItem {
            menu,
            ..HostTrayItem::default()
        }])
    };
    let empty = value(Vec::new());
    let one = value(vec![TrayMenuEntry::default()]);
    deepened(&empty, &one, levels)
}

fn submenus(levels: usize) -> usize {
    match decode_frame(&menu_frame(levels)).unwrap() {
        Message::Editor(EditorMessage::Menu { entries, .. }) => {
            let mut depth = 0;
            let mut entries = entries;
            while let Some(MenuEntry::Submenu(submenu)) = entries.pop() {
                depth += 1;
                entries = submenu.entries;
            }
            depth
        }
        _ => 0,
    }
}

fn tray_levels(levels: usize) -> Option<usize> {
    let items = decode_host::<Vec<HostTrayItem>>(&tray_value(levels))?;
    let mut depth = 0;
    let mut entries = items.into_iter().next()?.menu;
    while let Some(entry) = entries.pop() {
        depth += 1;
        entries = entry.children;
    }
    Some(depth)
}

#[test]
fn a_menu_nested_too_deeply_is_refused_while_decoding() {
    assert_eq!(
        submenus(MAX_MENU_DEPTH),
        MAX_MENU_DEPTH,
        "a menu as deep as the limit decodes"
    );
    for levels in [MAX_MENU_DEPTH + 1, 100_000] {
        assert_eq!(
            decode_frame(&menu_frame(levels)),
            Err(DecodeError::LimitExceeded("menu depth")),
            "a menu {levels} submenus deep is refused before it can overflow the stack"
        );
    }

    assert_eq!(
        tray_levels(MAX_MENU_DEPTH),
        Some(MAX_MENU_DEPTH),
        "a tray menu whose entries nest as deep as the limit decodes"
    );
    for levels in [MAX_MENU_DEPTH + 1, 100_000] {
        assert_eq!(
            tray_levels(levels),
            None,
            "a tray menu {levels} entries deep is refused before it can overflow the stack"
        );
    }
}
