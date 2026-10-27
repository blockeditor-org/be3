use super::*;

fn pane(depth: u32, key: &str, locked: bool) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Pane(DockPane {
            key: key.into(),
            active: Some(0),
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            locked,
            area: None,
        }),
    }
}

fn tab(depth: u32, path: DockTabPath) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Tab(path),
    }
}

fn declared(path: DockTabPath, key: &str) -> DockTab {
    DockTab {
        tab: path,
        key: key.into(),
        title: key.into(),
        glyph: String::new(),
        closable: true,
    }
}

fn declaration() -> DockDeclaration {
    let spliced = DockTabPath::own(4).through(ChildId(9));
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Split(DockSplit {
                        key: "main".into(),
                        direction: DockDirection::Horizontal,
                        fraction: 0.25,
                    }),
                },
                pane(1, "files", true),
                tab(2, DockTabPath::own(1)),
                pane(1, "editors", false),
                tab(2, DockTabPath::own(2)),
                DockItem {
                    depth: 2,
                    kind: DockItemKind::Group(DockGroup {
                        key: "editors-tab/0909/".into(),
                        pinned: true,
                        child: Some(ChildId(9)),
                    }),
                },
                pane(3, "editors-tab/0909/side", false),
                tab(4, spliced.clone()),
            ],
            focus: Some(DockTabPath::own(2)),
        },
        tabs: vec![
            declared(DockTabPath::own(1), "files"),
            declared(DockTabPath::own(2), "editor"),
            declared(spliced, "editors-tab/0909/side-tab"),
        ],
    }
}

#[test]
fn a_dock_declaration_round_trips_and_nests() {
    let declaration = declaration();
    let message = FrameCommit {
        docks: vec![DockMessage::Declare(DockDeclare {
            region: RegionRef {
                instance: EditorInstanceId(1),
                region: EditorRegion::Frame,
            },
            declaration: Some(declaration.clone()),
        })],
        ..frame(1)
    };
    round_trips(&message);
    assert_eq!(message.validate(), Ok(()));
    let nodes = declaration.nest().unwrap();
    assert_eq!(nodes.tabs().len(), 3);
    assert_eq!(DockTree::flatten(&nodes, declaration.tree.focus.clone()), declaration.tree);
    assert_eq!(
        DockTabPath::own(4).through(ChildId(9)).within(ChildId(9)),
        Some(DockTabPath::own(4))
    );

    let mut undescribed = declaration.clone();
    undescribed.tabs.pop();
    assert_eq!(undescribed.validate(), Err(DecodeError::MalformedPayload));

    let mut lopsided = declaration.clone();
    lopsided.tree.items.remove(1);
    lopsided.tree.items.remove(1);
    lopsided.tabs.remove(0);
    assert_eq!(lopsided.validate(), Err(DecodeError::MalformedPayload));

    let mut deep = declaration;
    deep.tree.items.last_mut().unwrap().kind = DockItemKind::Tab(DockTabPath {
        via: vec![ChildId(1); MAX_DOCK_NESTING + 1],
        tab: 4,
    });
    assert_eq!(deep.validate(), Err(DecodeError::LimitExceeded("dock nesting")));
}

#[test]
fn a_hosted_state_waiting_for_the_editor_is_replaced_by_the_latest() {
    let hosted = |stacked| {
        DockUpdate::Hosted(DockHostedRegion {
            region: EditorRegion::Frame,
            hosted: Some(DockHosted {
                layout: DockLayout {
                    tree: DockTree {
                        items: Vec::new(),
                        focus: None,
                    },
                    homes: Vec::new(),
                },
                tabs: Vec::new(),
                focused: None,
                shown: None,
                visible: Vec::new(),
                fullscreen: None,
                stacked,
            }),
        })
    };
    let close = DockUpdate::Close(DockClosedTab {
        region: EditorRegion::Frame,
        tab: DockTabPath::own(1),
    });
    let mut waiting = Vec::new();
    for update in [hosted(false), close.clone(), hosted(true)] {
        DockUpdate::queue(&mut waiting, update);
    }
    assert_eq!(waiting, [close, hosted(true)]);
    round_trips(&waiting);
}
