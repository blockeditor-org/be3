use super::*;
use block_plugin_api::{
    ChildContent, DockDeclaration, DockDeclaredChild, DockItem, DockItemKind, DockMessage,
    DockPane, DockTab, DockTabPath, DockTree, DockUpdate,
};

fn inner() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Pane(DockPane {
                        key: "inner".to_owned(),
                        active: None,
                        vertical: false,
                        icons_only: false,
                        sidebar: 180.0,
                        keep: false,
                        area: None,
                        empty: None,
                        bare: false,
                    }),
                },
                DockItem {
                    depth: 1,
                    kind: DockItemKind::Tab(DockTabPath::own(5)),
                },
            ],
            focus: None,
        },
        tabs: vec![DockTab {
            tab: DockTabPath::own(5),
            key: "5".to_owned(),
            title: "Inner".to_owned(),
            glyph: String::new(),
            closable: false,
            menu: Vec::new(),
        }],
        dialogs: Vec::new(),
        bars: Vec::new(),
    }
}

#[test]
fn a_root_dock_hosts_the_dock_its_child_declares_in_place_of_the_childs_tab() {
    let mut session = docked_session();
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], false);
    let child = session.placed_children(EditorRegion::Frame)[0].child;
    dock_messages(&mut session);

    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(inner()),
        })]);
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], false);
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], false);

    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    let state = layout.state().get_untracked();
    assert!(state.find_group("joined:1").is_some());
    assert!(!state.contains(beui::dock::unstyled::TabId::new(1)));
    let hosted: Vec<_> = dock_messages(&mut session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Host(host) if host.child.child == child => host.hosted,
            _ => None,
        })
        .collect();
    let hosted = hosted.last().expect("the root hosts its docked child");
    assert_eq!(hosted.tabs, [DockTabPath::own(5)]);
    assert_eq!(hosted.visible, [DockTabPath::own(5)]);
    let bodies: Vec<_> = session
        .placed_children(EditorRegion::Frame)
        .iter()
        .filter_map(|placed| match &placed.content {
            ChildContent::DockTab(tab) => Some(tab.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(bodies, [DockTabPath::own(5).through(child)]);
}
