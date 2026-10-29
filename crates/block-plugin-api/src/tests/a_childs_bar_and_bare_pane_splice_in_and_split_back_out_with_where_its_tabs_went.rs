use super::*;

fn barred() -> DockDeclaration {
    let mut declaration = child_declaration();
    if let DockItemKind::Pane(pane) = &mut declaration.tree.items[0].kind {
        pane.bare = true;
    }
    declaration.bars.push(DockBar {
        tab: DockTabPath::own(7),
        key: "tools".into(),
        owner: DockTabPath::own(4),
        placement: DockBarPlacement::Over(DockEdge::Top),
        size: Size {
            width: 200.0,
            height: 32.0,
        },
    });
    declaration
}

#[test]
fn a_childs_bar_and_bare_pane_splice_in_and_split_back_out_with_where_its_tabs_went() {
    let child = ChildId(9);
    let prefix = dock_prefix("nested", [9; 16]);
    let inner = barred();
    assert_eq!(inner.validate_own(), Ok(()));
    assert!(inner.shows_body(&DockTabPath::own(7)));
    round_trips(&inner);

    let mut orphaned = barred();
    orphaned.bars[0].owner = DockTabPath::own(5);
    assert_eq!(orphaned.validate(), Err(DecodeError::MalformedPayload));
    let mut clashing = barred();
    clashing.bars[0].key = "side-tab".into();
    assert_eq!(clashing.validate(), Err(DecodeError::MalformedPayload));

    let mut spliced = declaration();
    assert_eq!(spliced.splice(3, child, &prefix, &inner), Ok(true));
    assert_eq!(spliced.validate(), Ok(()));
    let side = DockTabPath::own(4).through(child);
    let tools = DockTabPath::own(7).through(child);
    let bar = spliced.bar(&tools).expect("the child's bar is spliced in");
    assert_eq!(bar.key, format!("{prefix}/tools"));
    assert_eq!(bar.owner, side);
    let bare = spliced.nest().unwrap();
    assert!(
        bare.panes()
            .iter()
            .any(|pane| pane.key == format!("{prefix}/side") && pane.bare)
    );

    let moved = DockTabPath::own(5).through(child);
    let hosted = DockHosted {
        layout: DockLayout {
            tree: spliced.tree.clone(),
            homes: Vec::new(),
            bars: vec![DockPlacedBar {
                bar: tools.clone(),
                owner: side.clone(),
                placement: DockBarPlacement::Window(DockBarWindow {
                    edge: DockEdge::Left,
                    origin: Pos2::new(40.0, 60.0),
                }),
            }],
            outside: vec![DockOutside {
                tab: moved,
                place: DockOutsidePlace::Pane(DockOutsidePane {
                    key: "editors".into(),
                    index: 1,
                }),
            }],
        },
        tabs: spliced.tabs.iter().map(|tab| tab.tab.clone()).collect(),
        focused: Some(side.clone()),
        shown: None,
        visible: vec![side],
        fullscreen: None,
        stacked: false,
    };
    round_trips(&hosted);
    assert_eq!(hosted.validate(), Ok(()));

    let own = hosted.own(&[(child, 3)]).unwrap();
    assert!(own.layout.bars.is_empty());
    assert!(own.layout.outside.is_empty());

    let part = hosted.child(child, &prefix).unwrap().unwrap();
    assert_eq!(part.layout.tree, inner.tree);
    assert_eq!(
        part.layout.bars,
        [DockPlacedBar {
            bar: DockTabPath::own(7),
            owner: DockTabPath::own(4),
            placement: DockBarPlacement::Window(DockBarWindow {
                edge: DockEdge::Left,
                origin: Pos2::new(40.0, 60.0),
            }),
        }]
    );
    assert_eq!(part.layout.outside[0].tab, DockTabPath::own(5));
    assert_eq!(
        part.layout.outside[0].place,
        DockOutsidePlace::Pane(DockOutsidePane {
            key: "editors".into(),
            index: 1,
        })
    );

    let restored = part.layout.through(child, &prefix).unwrap();
    assert_eq!(restored.bars[0].bar, tools);
    assert_eq!(restored.outside[0].tab, DockTabPath::own(5).through(child));
}
