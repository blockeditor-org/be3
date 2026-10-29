use super::*;
use block_plugin_api::{
    ChildContent, DockDeclaration, DockDeclaredChild, DockItem, DockItemKind, DockPane, DockTab,
    DockHold, DockMessage, DockTabPath, DockTree, DockUpdate,
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

fn declare(
    session: &mut EditorSession,
    child: block_plugin_api::ChildId,
    declared: bool,
) -> Vec<DockHold> {
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: declared.then(inner),
        })]);
    show_docked(session, &[(View::MAIN, 0)], false);
    show_docked(session, &[(View::MAIN, 0)], false);
    dock_messages(session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Host(host) if host.child.child == child => Some(host.hold),
            _ => None,
        })
        .collect()
}

fn block_child(session: &EditorSession) -> Option<block_plugin_api::ChildId> {
    session
        .placed_children(ViewId::MAIN)
        .iter()
        .find(|placed| matches!(placed.content, ChildContent::Block { .. }))
        .map(|placed| placed.child)
}

fn tab_bodies(session: &EditorSession) -> Vec<DockTabPath> {
    session
        .placed_children(ViewId::MAIN)
        .iter()
        .filter_map(|placed| match &placed.content {
            ChildContent::DockTab(tab) => Some(tab.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_root_hosts_a_childs_dock_again_after_the_child_withdraws_and_redeclares_it() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 0)], false);
    let child = block_child(&session).expect("the tab places its child");
    dock_messages(&mut session);
    declare(&mut session, child, true);
    assert_eq!(tab_bodies(&session), [DockTabPath::own(5).through(child)]);

    assert_eq!(
        declare(&mut session, child, false),
        [DockHold::Placing],
        "the root says it places the child in its tab again"
    );
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    assert!(
        layout
            .state()
            .get_untracked()
            .contains(beui::dock::unstyled::TabId::new(1)),
        "the child's own tab comes back while it declares nothing"
    );
    assert!(tab_bodies(&session).is_empty());
    assert_eq!(
        block_child(&session),
        Some(child),
        "the tab places the same child it placed before it was hosted"
    );

    declare(&mut session, child, true);
    assert_eq!(
        BODIES.with(std::cell::Cell::get),
        1,
        "the tab placing the child is built once and kept"
    );
    let state = layout.state().get_untracked();
    assert!(state.find_group("joined:1").is_some());
    assert_eq!(
        tab_bodies(&session),
        [DockTabPath::own(5).through(child)],
        "the root places the redeclared dock's tab bodies"
    );
}
