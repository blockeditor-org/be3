use super::*;

#[test]
fn the_palettes_split_actions_move_the_shown_tab_beside_its_pane() {
    let _root = crate::reactive::RootScope::new();
    for (id, beside) in [
        ("dock.tab.split-right", "right of"),
        ("dock.tab.split-down", "below"),
    ] {
        let (document, _) = dock_of(3);
        let mut harness = Harness::sized(document, WIDE_VIEWPORT);
        harness.frame(Vec::new());
        harness.click(harness.center(harness.find("dock.tab.2")));
        harness.frame(Vec::new());
        let split = dock_action(&harness, id);
        assert!(split.is_enabled(), "{id} splits a pane of several tabs");
        let before = harness.rect(harness.find("dock.tab.1"));

        assert!(split.run(), "{id} runs");
        harness.frame(Vec::new());
        let moved = harness.rect(harness.find("dock.tab.2"));
        let stayed = harness.rect(harness.find("dock.tab.1"));
        let content = harness.rect(harness.find("content.2"));
        match beside {
            "right of" => assert!(
                moved.min.x >= stayed.max.x && content.min.x >= stayed.max.x,
                "{id} moves the shown tab into a new pane {beside} its own: {moved:?}, {stayed:?}"
            ),
            _ => assert!(
                moved.min.y >= stayed.max.y && content.min.y >= stayed.max.y,
                "{id} moves the shown tab into a new pane {beside} its own: {moved:?}, {stayed:?}"
            ),
        }
        assert_eq!(
            stayed.min, before.min,
            "the other tabs stay in the pane they were in"
        );

        harness.click(harness.center(harness.find("dock.tab.2")));
        harness.frame(Vec::new());
        assert!(
            !dock_action(&harness, id).is_enabled(),
            "{id} does not split a pane showing a single tab"
        );
    }
}
