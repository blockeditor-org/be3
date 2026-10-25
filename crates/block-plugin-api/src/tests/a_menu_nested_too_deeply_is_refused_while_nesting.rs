use super::*;

fn menu_items(levels: usize) -> Vec<MenuItem> {
    (0..levels)
        .map(|depth| MenuItem {
            depth: depth as u32,
            kind: MenuItemKind::Submenu(MenuHeading::default()),
        })
        .collect()
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

fn tray_levels(levels: usize) -> Result<usize, DecodeError> {
    let item = HostTrayItem {
        menu: (0..levels)
            .map(|depth| TrayMenuItem {
                depth: depth as u32,
                ..TrayMenuItem::default()
            })
            .collect(),
        ..HostTrayItem::default()
    };
    let mut depth = 0;
    let mut entries = item.entries()?;
    while let Some(entry) = entries.pop() {
        depth += 1;
        entries = entry.children;
    }
    Ok(depth)
}

#[test]
fn a_menu_nested_too_deeply_is_refused_while_nesting() {
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
        tray_levels(MAX_MENU_DEPTH + 1),
        Ok(MAX_MENU_DEPTH + 1),
        "a tray menu whose entries nest as deep as the limit decodes"
    );
    for levels in [MAX_MENU_DEPTH + 2, MAX_COLLECTION_ITEMS + 1] {
        assert!(
            tray_levels(levels).is_err(),
            "a tray menu {levels} entries deep is refused"
        );
    }
}
