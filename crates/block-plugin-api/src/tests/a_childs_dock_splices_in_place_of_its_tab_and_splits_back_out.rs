use super::*;

#[test]
fn a_childs_dock_splices_in_place_of_its_tab_and_splits_back_out() {
    let child = ChildId(9);
    let prefix = dock_prefix("nested", [9; 16]);
    let side = DockTabPath::own(4).through(child);
    let mut spliced = declaration();
    assert_eq!(
        spliced.splice(3, child, &prefix, &child_declaration()),
        Ok(true)
    );
    assert_eq!(spliced.validate(), Ok(()));
    assert_eq!(spliced.tab(&DockTabPath::own(3)), None);
    assert_eq!(
        spliced.tab(&side).map(|tab| tab.key.clone()),
        Some(format!("{prefix}/side-tab"))
    );

    let hosted = DockHosted {
        layout: DockLayout {
            tree: spliced.tree.clone(),
            homes: Vec::new(),
            bars: Vec::new(),
            outside: Vec::new(),
            unlocked: Vec::new(),
        },
        tabs: spliced.tabs.iter().map(|tab| tab.tab.clone()).collect(),
        focused: Some(side.clone()),
        shown: None,
        visible: vec![DockTabPath::own(2), side.clone()],
        fullscreen: None,
        stacked: false,
        covers: Vec::new(),
    };
    let own = hosted.own(&[(child, 3)]).unwrap();
    assert_eq!(own.layout.tree, declaration().tree);
    assert_eq!(own.visible, [DockTabPath::own(2)]);
    assert_eq!(own.focused, Some(DockTabPath::own(3)));

    let inner = hosted.child(child, &prefix).unwrap().unwrap();
    assert_eq!(inner.layout.tree, child_declaration().tree);
    assert_eq!(inner.tabs, [DockTabPath::own(4)]);
    assert_eq!(inner.visible, [DockTabPath::own(4)]);
    assert_eq!(inner.focused, Some(DockTabPath::own(4)));

    assert_eq!(
        DockRequest::Show(DockTabPath::own(4)).through(child, &prefix),
        Ok(DockRequest::Show(side))
    );
}
