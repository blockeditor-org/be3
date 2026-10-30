use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_bare_dock_pane_draws_no_tab_bar() {
    let _root = crate::reactive::RootScope::new();
    let (document, _) = bar_dock(
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        true,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.tab.1").is_none(),
        "the bare pane has no tab for its tab"
    );
    assert!(harness.document().find_test_id("dock.grip.stage").is_none());
    assert!(harness.document().find_test_id("dock.tab.2").is_some());
    let stage = harness.rect(harness.find("content.1"));
    let side = harness.rect(harness.find("content.2"));
    assert!(
        stage.top() < side.top(),
        "the bare pane's body reaches up where a tab bar would be"
    );
}
