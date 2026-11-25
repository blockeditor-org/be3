use super::*;

/// The child of editor 1 that `placement` places, asked for that place as a settle asks it.
fn asked(placement: ChildPlacement) -> Child {
    let child = wanted(placement.clone());
    *child.editor.asks.borrow_mut() = Some(PlaceAsk {
        placement,
        raster: 1.0,
        framing: None,
        space: None,
    });
    child
}

#[test]
fn a_child_in_a_dock_tabs_body_is_docked_before_its_editor_is_first_asked_for_screens() {
    // Editor 1 places block 1 in the body of its dock's tab 3: its editor is the one the docks
    // must know before it is first asked for its screens.
    let mut tabbed = placed(rect((0.0, 0.0), (300.0, 200.0)), ChildMode::Live);
    tabbed.docks = Some(3);
    let block = Uuid::from_bytes([1; 16]);
    assert_eq!(asked(tabbed.clone()).editor.in_tab(), Some(block));

    // Known to them as the root's settle makes it known (`register_in_tab`), before it shows,
    // it is docked, so its first screens ask for its frame docked.
    let mut docks = Docks::default();
    docks.register(EditorInstanceId(1), &app_key(5), Some(Uuid::from_bytes([9; 16])));
    docks.register(EditorInstanceId(2), &app_key(6), Some(block));
    assert_eq!(docks.block_editor(block), Some(EditorInstanceId(2)));
    let placements = [ChildPlacements {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        children: vec![tabbed.clone()],
        occluders: Vec::new(),
    }];
    let docked = docked_children(&placements);
    docks.placed(EditorInstanceId(1), &docked[&EditorInstanceId(1)], None);
    assert!(docks.docked(EditorInstanceId(2)));

    // One placed as a preview has no dock, and one placed on the canvas is not in a tab.
    let mut preview = tabbed.clone();
    preview.mode = ChildMode::Preview;
    assert_eq!(asked(preview).editor.in_tab(), None);
    let mut card = tabbed;
    card.docks = None;
    assert_eq!(asked(card).editor.in_tab(), None);
}
