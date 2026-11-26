use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_joined_docks_bar_drops_on_any_edge_of_any_of_its_panes() {
    let _root = crate::reactive::RootScope::new();
    let (document, LayeredDock { outer, .. }) = layered_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let bar = harness.rect(harness.find("bar.middle"));
    let left = harness.rect(harness.find("content.10"));
    assert!(
        left.contains(bar.center()),
        "the middle layer's bar starts over its dock's first pane: {bar:?} in {left:?}"
    );

    // The bar docks on every edge of the middle layer's panes and of the inner layer's, which it
    // reaches through the dock the middle layer's second pane hosts. The top of the inner
    // layer's first pane, where its own bar is, comes last.
    for (pane, edge) in [
        ("content.10", DockEdge::Bottom),
        ("content.10", DockEdge::Left),
        ("content.10", DockEdge::Right),
        ("content.10", DockEdge::Top),
        ("content.21", DockEdge::Bottom),
        ("content.21", DockEdge::Left),
        ("content.21", DockEdge::Right),
        ("content.21", DockEdge::Top),
        ("content.20", DockEdge::Bottom),
        ("content.20", DockEdge::Left),
        ("content.20", DockEdge::Right),
    ] {
        let body = harness.rect(harness.find(pane));
        let grip = grip_of(&harness, "bar.middle");
        harness.drag_and_rest(grip, inside(edge, body));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let body = harness.rect(harness.find(pane));
        let bar = harness.rect(harness.find("bar.middle"));
        assert!(
            along(edge, bar, body),
            "dropped at the {edge:?} edge of {pane}, the bar docks beside it: {bar:?} by {body:?}, {:?}",
            middle_bar(&outer)
        );
        assert_eq!(
            middle_bar(&outer).1,
            Some(DockBarPlacement::Beside(edge, DockAlign::Center)),
            "at the {edge:?} edge of {pane}"
        );
    }

    // On the inner layer's pane, the middle layer's bar lands among the inner layer's bars where
    // it is dropped, and they among it.
    let inner = harness.rect(harness.find("bar.inner"));
    let grip = grip_of(&harness, "bar.middle");
    harness.drag_and_rest(grip, pos2(inner.right() + 30.0, inner.center().y));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        side_by_side(&harness, "bar.inner", "bar.middle"),
        "dropped after the inner layer's bar, the middle layer's bar lands after it: {:?}",
        middle_bar(&outer)
    );
    let middle = harness.rect(harness.find("bar.middle"));
    let grip = grip_of(&harness, "bar.inner");
    harness.drag_and_rest(grip, pos2(middle.right() + 30.0, middle.center().y));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        side_by_side(&harness, "bar.middle", "bar.inner"),
        "the inner layer's bar dropped after the middle layer's lands after it"
    );
}
