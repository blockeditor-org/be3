use super::*;

#[test]
fn the_palettes_close_tab_action_closes_the_tab_the_focused_pane_shows() {
    let _root = crate::reactive::RootScope::new();
    let (document, _) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("dock.tab.2")));
    harness.frame(Vec::new());
    let close = dock_action(&harness, "dock.tab.close");
    assert!(close.is_enabled(), "a tab with on_close can be closed");

    assert!(close.run());
    harness.frame(Vec::new());
    let document = harness.document();
    assert!(
        document.find_test_id("dock.tab.2").is_none() && document.find_test_id("content.2").is_none(),
        "Close tab closes the tab the focused pane shows"
    );
    assert!(
        document.find_test_id("dock.tab.1").is_some() && document.find_test_id("dock.tab.3").is_some(),
        "and only that one"
    );
}
