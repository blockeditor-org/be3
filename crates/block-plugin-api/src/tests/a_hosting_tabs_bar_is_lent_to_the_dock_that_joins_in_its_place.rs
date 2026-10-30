use super::*;

#[test]
fn a_hosting_tabs_bar_is_lent_to_the_dock_that_joins_in_its_place() {
    let child = ChildId(9);
    let prefix = dock_prefix("nested", [9; 16]);
    let toolbar = DockBar {
        tab: DockTabPath::own(8),
        key: "toolbar".into(),
        kind: "canvas/toolbar".to_owned(),
        owner: Some(DockTabPath::own(3)),
        placement: DockBarPlacement::Beside(DockBarEdge {
 edge: DockEdge::Top,
 align: DockAlign::Center,
 }),
        size: Size {
            width: 200.0,
            height: 40.0,
        },
    };
    let mut spliced = declaration();
    spliced.bars.push(toolbar.clone());
    assert_eq!(
        spliced.splice(3, child, &prefix, &child_declaration()),
        Ok(true)
    );
    assert_eq!(spliced.validate(), Ok(()));
    let side = DockTabPath::own(4).through(child);
    assert_eq!(
        spliced
            .bar(&DockTabPath::own(8))
            .map(|bar| bar.owner.as_ref()),
        Some(Some(&side)),
        "the bar stays, on the pane of the first tab that joined in its tab's place"
    );

    let hosted = DockHosted {
        layout: DockLayout {
            tree: spliced.tree.clone(),
            homes: Vec::new(),
            unlocked: Vec::new(),
            bars: vec![DockPlacedBar {
                bar: DockTabPath::own(8),
                owner: Some(side),
                placement: DockBarPlacement::Beside(DockBarEdge {
 edge: DockEdge::Left,
 align: DockAlign::Center,
 }),
            }],
            outside: Vec::new(),
        },
        tabs: spliced.tabs.iter().map(|tab| tab.tab.clone()).collect(),
        focused: None,
        shown: None,
        visible: Vec::new(),
        fullscreen: None,
        stacked: false,
        covers: Vec::new(),
    };
    assert_eq!(
        hosted.own(&[(child, 3)]).unwrap().layout.bars,
        [DockPlacedBar {
            bar: DockTabPath::own(8),
            owner: Some(DockTabPath::own(3)),
            placement: DockBarPlacement::Beside(DockBarEdge {
 edge: DockEdge::Left,
 align: DockAlign::Center,
 }),
        }],
        "its owner hears it back on its own tab"
    );
    assert!(
        hosted
            .child(child, &prefix)
            .unwrap()
            .unwrap()
            .layout
            .bars
            .is_empty()
    );
}
