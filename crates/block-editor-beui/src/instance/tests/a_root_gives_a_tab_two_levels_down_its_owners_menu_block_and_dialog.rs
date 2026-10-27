use super::*;
use beui::reactive::{menu_action, untrack};
use block_plugin_api::{
    ChildContent, ChildId, DockDeclaration, DockDeclaredChild, DockDialog, DockItem, DockItemKind,
    DockMessage, DockPane, DockTab, DockTabPath, DockTree, DockUpdate, MenuCheck, MenuCommand,
    MenuEntry, MenuPin, Size, dock_prefix,
};

const NESTED: Uuid = Uuid::from_u128(0x77);
const NESTED_CHILD: ChildId = ChildId(40);

fn one(key: &str, tab: u64, menu: &[&str]) -> DockDeclaration {
    let entries: Vec<MenuEntry> = menu
        .iter()
        .map(|id| {
            MenuEntry::Command(MenuCommand {
                id: (*id).to_owned(),
                label: (*id).to_owned(),
                glyph: String::new(),
                enabled: true,
                check: MenuCheck::None,
                shortcut: None,
                pin: MenuPin::None,
            })
        })
        .collect();
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Pane(DockPane {
                        key: key.to_owned(),
                        active: None,
                        vertical: false,
                        icons_only: false,
                        sidebar: 180.0,
                        keep: false,
                        area: None,
                        empty: None,
                    }),
                },
                DockItem {
                    depth: 1,
                    kind: DockItemKind::Tab(DockTabPath::own(tab)),
                },
            ],
            focus: None,
        },
        tabs: vec![DockTab {
            tab: DockTabPath::own(tab),
            key: tab.to_string(),
            title: key.to_owned(),
            glyph: String::new(),
            closable: false,
            menu: MenuEntry::flatten(&entries),
        }],
        dialogs: Vec::new(),
    }
}

#[test]
fn a_root_gives_a_tab_two_levels_down_its_owners_menu_block_and_dialog() {
    let mut session = docked_session();
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], false);
    let child = session.placed_children(EditorRegion::Frame)[0].child;
    dock_messages(&mut session);

    let mut deep = one("deep", 7, &["deep.act"]);
    deep.dialogs.push(DockDialog {
        tab: DockTabPath::own(8),
        parent: DockTabPath::own(7),
        title: "Deep dialog".to_owned(),
        glyph: String::new(),
        size: Size {
            width: 160.0,
            height: 90.0,
        },
        modal: false,
        card: false,
        dismiss_on_outside_press: false,
        anchor: None,
        closable: true,
    });
    let mut middle = one("middle", 6, &[]);
    let prefix = dock_prefix("6", NESTED.into_bytes());
    assert_eq!(middle.splice(6, NESTED_CHILD, &prefix, &deep), Ok(true));
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(middle),
        })]);
    for _ in 0..3 {
        show_docked(&mut session, &[(EditorRegion::Frame, 0)], false);
    }

    let deep_tab = DockTabPath {
        via: vec![NESTED_CHILD],
        tab: 7,
    };
    let tabs = DOCK_TABS.with(|tabs| tabs.borrow().clone());
    let (owner, menu) = tabs
        .iter()
        .rev()
        .find(|(owner, _)| owner.is_some())
        .copied()
        .expect("the root frames the tab two levels down");
    assert_eq!(
        owner,
        Some(NESTED),
        "the tab carries the block that owns it, not the child that placed it"
    );
    let action = menu_action(&menu.get_untracked(), "deep.act")
        .expect("the tab two levels down carries its owner's menu");
    dock_messages(&mut session);
    untrack(|| action.run());
    let picks: Vec<_> = dock_messages(&mut session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Pick(pick) => Some((pick.child.child, pick.tab, pick.id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        picks,
        [(child, deep_tab.clone(), "deep.act".to_owned())],
        "a pick goes to the tab's owner through the child"
    );

    let bodies: Vec<_> = session
        .placed_children(EditorRegion::Frame)
        .iter()
        .filter_map(|placed| match &placed.content {
            ChildContent::DockTab(tab) => Some(tab.clone()),
            _ => None,
        })
        .collect();
    assert!(
        bodies.contains(&deep_tab.clone().through(child)),
        "the deep tab's body is placed: {bodies:?}"
    );
    assert!(
        bodies.contains(
            &DockTabPath {
                via: vec![NESTED_CHILD],
                tab: 8,
            }
            .through(child)
        ),
        "the deep dock's dialog is presented by the root, its body placed: {bodies:?}"
    );
}
