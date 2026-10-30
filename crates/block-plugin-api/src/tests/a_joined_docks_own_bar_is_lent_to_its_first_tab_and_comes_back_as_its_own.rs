use super::*;

#[test]
fn a_joined_docks_own_bar_is_lent_to_its_first_tab_and_comes_back_as_its_own() {
    let child = ChildId(9);
    let prefix = dock_prefix("nested", [9; 16]);
    let mut inner = child_declaration();
    inner.bars.push(DockBar {
        tab: DockTabPath::own(6),
        key: "apps".into(),
        kind: "desktop/apps".to_owned(),
        owner: None,
        placement: DockBarPlacement::Beside(DockEdge::Bottom),
        size: Size {
            width: 40.0,
            height: 40.0,
        },
    });
    assert_eq!(
        inner.validate_own(),
        Ok(()),
        "a dock declares a bar of its own with no owner"
    );
    let mut spliced = declaration();
    assert_eq!(spliced.splice(3, child, &prefix, &inner), Ok(true));
    assert_eq!(spliced.validate(), Ok(()));
    let bar = DockTabPath::own(6).through(child);
    let side = DockTabPath::own(4).through(child);
    assert_eq!(
        spliced.bar(&bar).map(|bar| bar.owner.as_ref()),
        Some(Some(&side)),
        "the root gives it to the first tab of the group that joined"
    );

    let hosted = DockHosted {
        layout: DockLayout {
            tree: spliced.tree.clone(),
            homes: Vec::new(),
            unlocked: Vec::new(),
            bars: vec![DockPlacedBar {
                bar,
                owner: Some(side),
                placement: DockBarPlacement::Beside(DockEdge::Top),
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
    let mut part = hosted
        .child(child, &prefix)
        .unwrap()
        .expect("the child's part of what the root hosts");
    part.own_dock_bars(&inner);
    assert_eq!(
        part.layout.bars,
        [DockPlacedBar {
            bar: DockTabPath::own(6),
            owner: None,
            placement: DockBarPlacement::Beside(DockEdge::Top),
        }],
        "the child hears it back as its dock's own"
    );
    assert_eq!(part.validate(), Ok(()));
}
