//! What the tests of a descendant's restore share: a child whose dock splices in a dock of a
//! child of its own, and the restore that nested dock asks for.

use super::*;
use be_sdk_api::{
    ChildId, DockChildRequest, DockDeclaration, DockDeclaredChild, DockGroup, DockHold, DockHosted,
    DockItem, DockItemKind, DockLayout, DockMessage, DockPane, DockRequest, DockTab, DockTabPath,
    DockTree, DockUpdate,
};

pub(super) const NESTED: ChildId = ChildId(77);

pub(super) fn pane(depth: u32, key: &str, icons_only: bool) -> DockItem {
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

pub(super) fn tab(depth: u32, tab: DockTabPath) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Tab(tab),
    }
}

pub(super) fn nested_tab() -> DockTabPath {
    DockTabPath {
        via: vec![NESTED],
        tab: 7,
    }
}

pub(super) fn dock_tab(tab: DockTabPath, key: &str) -> DockTab {
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
pub(super) fn spliced() -> DockDeclaration {
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
pub(super) fn restore() -> DockRequest {
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

pub(super) fn last_hold(session: &mut EditorSession) -> Option<DockHosted> {
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

pub(super) fn icons_only(hosted: &DockHosted) -> bool {
    hosted.layout.tree.items.iter().any(|item| {
        matches!(
            &item.kind,
            DockItemKind::Pane(pane) if pane.key == "nested/inner" && pane.icons_only
        )
    })
}

/// The child's dock once it no longer splices the nested dock in.
pub(super) fn unspliced() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![pane(0, "outer", false), tab(1, DockTabPath::own(5))],
            focus: None,
        },
        tabs: vec![dock_tab(DockTabPath::own(5), "five")],
        dialogs: Vec::new(),
        bars: Vec::new(),
    }
}

/// Places the docked child and hears it declare `declaration` and ask for the nested dock's
/// restore, before the root hosts anything of it. Answers the child.
pub(super) fn ask_to_restore(session: &mut EditorSession) -> ChildId {
    show_docked(session, &[(View::MAIN, 6)], false);
    let child = session.placed_children(ViewId::MAIN)[0].child;
    dock_messages(session);
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
    child
}

/// The root's last hold of its child over a few frames.
pub(super) fn settled_hold(session: &mut EditorSession) -> DockHosted {
    let mut hosted = None;
    for _ in 0..3 {
        show_docked(session, &[(View::MAIN, 6)], false);
        hosted = last_hold(session).or(hosted);
    }
    hosted.expect("the root hosts its child")
}
