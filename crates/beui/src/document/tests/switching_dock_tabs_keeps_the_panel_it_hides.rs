use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn switching_dock_tabs_keeps_the_panel_it_hides() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let first = harness.find("content.1");
    let second = harness.find("content.2");
    assert!(
        harness.rect(first).is_positive(),
        "the pane lays out the tab it is showing"
    );
    assert!(
        harness.document().node_rect(second).is_none(),
        "the pane does not lay out the tab it is not showing"
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
        harness.rect(second).is_positive(),
        "the panel that was hidden is laid out once its tab is shown"
    );
}
