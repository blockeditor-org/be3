use super::*;

fn bar(tab: u64, key: &str, owner: u64, edge: DockEdge) -> DockBar {
    DockBar {
        tab: DockTabPath::own(tab),
        key: key.into(),
        kind: format!("canvas/{key}"),
        owner: DockTabPath::own(owner),
        placement: DockBarPlacement::Over(edge),
        size: Size {
            width: 200.0,
            height: 32.0,
        },
    }
}

#[test]
fn a_frame_childs_bars_splice_in_place_of_its_parents_and_split_back_out() {
    let child = ChildId(9);
    let prefix = dock_prefix("editor", [9; 16]);
    let mut inner = child_declaration();
    inner.bars.push(bar(7, "tools", 4, DockEdge::Bottom));
    assert_eq!(inner.validate_own(), Ok(()));

    let mut spliced = declaration();
    spliced.bars.push(bar(8, "tools", 2, DockEdge::Left));
    spliced.bars.push(bar(9, "tags", 1, DockEdge::Top));
    spliced.splice_bars(Some(2), child, &prefix, &inner);
    assert_eq!(spliced.validate(), Ok(()));
    assert_eq!(
        spliced.tabs,
        declaration().tabs,
        "the child's tabs stay out"
    );
    let tools = DockTabPath::own(7).through(child);
    let keys: Vec<(&DockTabPath, &str, &DockTabPath)> = spliced
        .bars
        .iter()
        .map(|bar| (&bar.tab, bar.key.as_str(), &bar.owner))
        .collect();
    let joined = format!("{prefix}/tools");
    assert_eq!(
        keys,
        [
            (&DockTabPath::own(9), "tags", &DockTabPath::own(1)),
            (&tools, joined.as_str(), &DockTabPath::own(2)),
        ],
        "the parent's bars on the taken tab go, the child's join that tab"
    );

    let mut nested = inner.clone();
    nested.splice_bars(None, ChildId(3), "inner", &inner);
    assert_eq!(
        nested
            .bars
            .iter()
            .map(|bar| bar.owner.clone())
            .collect::<Vec<_>>(),
        [DockTabPath::own(4).through(ChildId(3))],
        "spliced as bars only, every bar of the dock goes and the child's keep their owners"
    );

    let hosted = DockHosted {
        layout: DockLayout {
            tree: spliced.tree.clone(),
            homes: Vec::new(),
            unlocked: Vec::new(),
            bars: vec![DockPlacedBar {
                bar: tools.clone(),
                owner: DockTabPath::own(2),
                placement: DockBarPlacement::Beside(DockEdge::Right),
            }],
            outside: Vec::new(),
        },
        tabs: spliced.tabs.iter().map(|tab| tab.tab.clone()).collect(),
        focused: Some(DockTabPath::own(2)),
        shown: None,
        visible: vec![DockTabPath::own(2)],
        fullscreen: None,
        stacked: false,
        covers: vec![
            DockCover {
                tab: DockTabPath::own(2),
                rect: Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)),
            },
            DockCover {
                tab: DockTabPath::own(1),
                rect: Rect::from_min_max(Pos2::new(5.0, 5.0), Pos2::new(8.0, 8.0)),
            },
        ],
    };
    let part = hosted.frame_child(child, Some(2), &|bar| {
        inner.bar(bar).map(|bar| bar.owner.clone())
    });
    assert_eq!(hosted.own(&[]).unwrap().layout.bars, []);
    assert_eq!(
        part.layout.bars,
        [DockPlacedBar {
            bar: DockTabPath::own(7),
            owner: DockTabPath::own(4),
            placement: DockBarPlacement::Beside(DockEdge::Right),
        }]
    );
    assert_eq!(
        part.covers,
        [DockCover {
            tab: DockTabPath::own(4),
            rect: Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)),
        }],
        "the child keeps clear of what covers the tab whose frame it took"
    );
    assert_eq!(part.validate(), Ok(()));
}
