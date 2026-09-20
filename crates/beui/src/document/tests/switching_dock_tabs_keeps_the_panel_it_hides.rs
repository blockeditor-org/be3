use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn switching_dock_tabs_keeps_the_panel_it_hides() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let first = harness.find("content.1");
    assert!(
        harness.rect(first).is_positive(),
        "the pane lays out the tab it is showing"
    );
    assert!(
        harness.document().find_test_id("content.2").is_none(),
        "the panel of a tab that has not been shown is not built yet"
    );

    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 2")));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(2)),
        "clicking a tab shows the panel it names"
    );
    assert_eq!(
        harness.find("content.1"),
        first,
        "the panel that is now hidden keeps the nodes it had"
    );
    assert!(
        harness.document().node_rect(first).is_none(),
        "the pane does not lay out the tab it stopped showing"
    );
    assert!(
        harness.rect(harness.find("content.2")).is_positive(),
        "the panel of the tab it shows now is laid out in its place"
    );
}
