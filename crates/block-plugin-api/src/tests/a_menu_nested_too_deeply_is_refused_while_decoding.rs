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

fn menu_items(levels: usize) -> Vec<MenuItem> {
    (0..levels)
        .map(|depth| MenuItem {
            depth: depth as u32,
            kind: MenuItemKind::Submenu(MenuHeading::default()),
        })
        .collect()
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
    let mut entries = MenuEntry::nest(menu_items(levels)).unwrap();
    let mut depth = 0;
    while let Some(MenuEntry::Submenu(submenu)) = entries.pop() {
        depth += 1;
        entries = submenu.entries;
    }
    depth
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
    for levels in [MAX_MENU_DEPTH + 1, MAX_COLLECTION_ITEMS] {
        assert_eq!(
            MenuEntry::nest(menu_items(levels)),
            Err(DecodeError::LimitExceeded("menu depth")),
            "a menu {levels} submenus deep is refused"
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
