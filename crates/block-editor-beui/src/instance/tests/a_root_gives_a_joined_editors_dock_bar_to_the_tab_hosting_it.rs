use super::*;
use block_plugin_api::{
    DockBar, DockBarPlacement, DockDeclaration, DockDeclaredChild, DockEdge, DockHold, DockItem,
    DockItemKind, DockMessage, DockPane, DockTab, DockTabPath, DockTree, DockUpdate, Size,
};

fn declared() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Pane(DockPane {
                        key: "stage".to_owned(),
                        active: None,
                        vertical: false,
                        icons_only: false,
                        sidebar: 180.0,
                        keep: false,
                        area: None,
                        empty: None,
                        bare: true,
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
            title: "Stage".to_owned(),
            glyph: String::new(),
            closable: false,
            menu: Vec::new(),
        }],
        dialogs: Vec::new(),
        bars: vec![DockBar {
            tab: DockTabPath::own(9),
            key: "apps".to_owned(),
            kind: "desktop/apps".to_owned(),
            owner: None,
            placement: DockBarPlacement::Beside(DockEdge::Bottom),
            size: Size {
                width: 100.0,
                height: 20.0,
            },
        }],
    }
}

#[test]
fn a_root_gives_a_joined_editors_dock_bar_to_the_tab_hosting_it() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 6)], false);
    let child = session.placed_children(ViewId::MAIN)[0].child;
    dock_messages(&mut session);
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(declared()),
        })]);
    show_docked(&mut session, &[(View::MAIN, 6)], false);
    show_docked(&mut session, &[(View::MAIN, 6)], false);

    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    let state = layout.state().get_untracked();
    let bars = state.bars();
    let [bar] = bars.as_slice() else {
        panic!("the root holds the child's one bar");
    };
    assert!(
        state.bars_of(None).is_empty(),
        "a joined editor's own bar is not the root dock's"
    );
    assert_eq!(
        state.bar_placement(*bar),
        Some(beui::dock::unstyled::DockBarPlacement::Beside(
            beui::dock::unstyled::DockEdge::Bottom
        ))
    );
    let hosted = dock_messages(&mut session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Host(host) => match host.hold {
                DockHold::Hosted(hosted) => Some(hosted),
                _ => None,
            },
            _ => None,
        })
        .last()
        .expect("the root hosts its child");
    assert_eq!(hosted.layout.bars.len(), 1);
    assert_eq!(hosted.layout.bars[0].bar, DockTabPath::own(9));
    assert_eq!(
        hosted.layout.bars[0].owner, None,
        "the child hears its bar back as its dock's own"
    );
}
