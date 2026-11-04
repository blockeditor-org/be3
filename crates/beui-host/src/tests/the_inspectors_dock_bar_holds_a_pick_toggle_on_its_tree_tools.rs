use super::*;

#[test]
fn the_inspectors_dock_bar_holds_a_pick_toggle_on_its_tree_tools() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    let tools = harness.inspector_rect("inspector.dock");
    harness.click(harness.inspector_center("dock.tools.pin.inspector.pick"));
    harness.settle();
    assert!(
        harness.inspector().state().picking.get(),
        "the pin turns picking on"
    );
    harness.click(harness.inspector_center("dock.tools.pin.inspector.pick"));
    harness.settle();
    assert!(!harness.inspector().state().picking.get(), "and off again");

    harness.show_inspector_tab(crate::inspector::InspectorTab::AccessKit);
    assert!(
        harness
            .inspector()
            .document
            .find_test_id("dock.tools.pin.inspector.pick")
            .is_some(),
        "every tree tool offers Pick"
    );
    harness.show_inspector_tab(crate::inspector::InspectorTab::Performance);
    assert!(
        harness
            .inspector()
            .document
            .find_test_id("dock.tools.pin.inspector.pick")
            .is_none(),
        "the performance tool offers no Pick"
    );
    assert_eq!(
        harness.inspector_rect("inspector.dock"),
        tools,
        "the dock stays put when the tool changes"
    );
}
