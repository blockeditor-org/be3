use super::*;
use be_sdk_api::{
    ChildId, DockChildRequest, DockDeclaration, DockDeclaredChild, DockGroup, DockHold, DockHosted,
    DockItem, DockItemKind, DockLayout, DockMessage, DockPane, DockRequest, DockTab, DockTabPath,
    DockTree, DockUpdate,
};

const NESTED: ChildId = ChildId(77);

fn pane(depth: u32, key: &str, icons_only: bool) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Pane(DockPane {
            key: key.to_owned(),
            active: None,
            vertical: false,
            icons_only,
            sidebar: 180.0,
            keep: false,
            area: None,
            empty: None,
            bare: false,
            hide_empty_bar: false,
        }),
    }
}

fn tab(depth: u32, tab: DockTabPath) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Tab(tab),
    }
}

fn nested_tab() -> DockTabPath {
    DockTabPath {
        via: vec![NESTED],
        tab: 7,
    }
}

fn dock_tab(tab: DockTabPath, key: &str) -> DockTab {
    DockTab {
        tab,
        key: key.to_owned(),
        title: key.to_owned(),
        glyph: String::new(),
        closable: false,
        back_closes: true,
        unread: 0,
        menu: Vec::new(),
    }
}

/// The child's dock with the dock of a child of its own spliced in as a group.
fn spliced() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![
                pane(0, "outer", false),
                tab(1, DockTabPath::own(5)),
                DockItem {
                    depth: 1,
                    kind: DockItemKind::Group(DockGroup {
                        key: "nested".to_owned(),
                        pinned: true,
                        child: Some(NESTED),
                        title: String::new(),
                    }),
                },
                pane(2, "nested/inner", false),
                tab(3, nested_tab()),
            ],
            focus: None,
        },
        tabs: vec![
            dock_tab(DockTabPath::own(5), "five"),
            dock_tab(nested_tab(), "nested/seven"),
        ],
        dialogs: Vec::new(),
        bars: Vec::new(),
    }
}

/// What the nested dock asks to restore: its pane showing icons only.
fn restore() -> DockRequest {
    DockRequest::Restore(DockLayout {
        tree: DockTree {
            items: vec![pane(0, "nested/inner", true), tab(1, nested_tab())],
            focus: None,
        },
        homes: Vec::new(),
        unlocked: Vec::new(),
        compact: Vec::new(),
        auto_hidden: Vec::new(),
        bars: Vec::new(),
        outside: Vec::new(),
    })
}

fn last_hold(session: &mut EditorSession) -> Option<DockHosted> {
    dock_messages(session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Host(host) => match host.hold {
                DockHold::Hosted(hosted) => Some(hosted),
                _ => None,
            },
            _ => None,
        })
        .next_back()
}

fn icons_only(hosted: &DockHosted) -> bool {
    hosted.layout.tree.items.iter().any(|item| {
        matches!(
            &item.kind,
            DockItemKind::Pane(pane) if pane.key == "nested/inner" && pane.icons_only
        )
    })
}

#[test]
fn a_root_takes_a_descendants_restore_once_it_hosts_its_group_and_counts_it() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 6)], false);
    let child = session.placed_children(ViewId::MAIN)[0].child;
    dock_messages(&mut session);
    // The restore comes with the declaration that splices its group in, before the root hosts
    // the group.
    session.host().receive_dock_updates(vec![
        DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(spliced()),
            epoch: 1,
        }),
        DockUpdate::ChildRequest(DockChildRequest {
            child,
            owner: vec![NESTED],
            request: restore(),
        }),
    ]);
    let mut hosted = None;
    for _ in 0..3 {
        show_docked(&mut session, &[(View::MAIN, 6)], false);
        hosted = last_hold(&mut session).or(hosted);
    }
    let hosted = hosted.expect("the root hosts its child");
    assert!(
        icons_only(&hosted),
        "the root restores the nested dock's group once it hosts it"
    );
    assert_eq!(hosted.restored, 1, "the root's hold counts the restore it took");
}
