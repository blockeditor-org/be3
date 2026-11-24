use super::*;

#[test]
fn a_child_placed_in_a_dock_tabs_body_joins_its_placers_dock() {
    // Editor 1 places block 7 in the body of its dock's tab 3, and a card on its canvas.
    let mut tabbed = placed(rect((0.0, 0.0), (300.0, 200.0)), ChildMode::Live);
    tabbed.docks = Some(3);
    let mut card = placed(rect((10.0, 10.0), (60.0, 60.0)), ChildMode::Passive);
    card.child = ChildId(8);
    let placements = [ChildPlacements {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        children: vec![tabbed, card],
        occluders: Vec::new(),
    }];
    let docked = docked_children(&placements);
    let children = &docked[&EditorInstanceId(1)];
    assert_eq!(children.len(), 1);
    assert_eq!((children[0].child, children[0].tab), (ChildId(7), 3));

    // Its docks then ask the child, once its editor shows the block, for its frame docked.
    let mut docks = Docks::default();
    docks.register(EditorInstanceId(1), &app_key(5), Some(Uuid::from_bytes([9; 16])));
    docks.register(EditorInstanceId(2), &app_key(6), Some(Uuid::from_bytes([1; 16])));
    docks.placed(EditorInstanceId(1), children, None);
    assert!(docks.docked(EditorInstanceId(2)));
    assert!(!docks.docked(EditorInstanceId(1)));
}
