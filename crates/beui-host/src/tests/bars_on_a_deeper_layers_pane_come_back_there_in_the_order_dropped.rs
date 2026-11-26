use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

fn settle(harness: &mut Harness) {
    for _ in 0..3 {
        harness.frame(Vec::new());
    }
}

fn snapshots(layers: &LayeredDock) -> LayeredSnapshots {
    [
        layers.outer.snapshot(),
        layers.middle.snapshot(),
        layers.inner.snapshot(),
    ]
}

/// The middle layer's bar dropped on panes of the inner layer, joined below its own, comes back
/// on the same pane once each layer has saved its layout apart and restored it; and on a pane
/// holding bars of both layers, they come back in the order dropped, not outer layers first.
#[test]
fn bars_on_a_deeper_layers_pane_come_back_there_in_the_order_dropped() {
    let _root = crate::reactive::RootScope::new();
    let (document, layers) = layered_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    settle(&mut harness);

    // The middle layer's bar docks at the bottom of the inner layer's second pane.
    let body = harness.rect(harness.find("content.21"));
    let grip = grip_of(&harness, "bar.middle");
    harness.drag_and_rest(grip, inside(DockEdge::Bottom, body));
    settle(&mut harness);
    let body = harness.rect(harness.find("content.21"));
    let bar = harness.rect(harness.find("bar.middle"));
    assert!(
        along(DockEdge::Bottom, bar, body),
        "dropped at the bottom of the inner layer's second pane: {bar:?} by {body:?}"
    );

    let saved = snapshots(&layers);
    drop(harness);
    let (document, _layers) = layered_dock(Some(saved));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    settle(&mut harness);
    let body = harness.rect(harness.find("content.21"));
    let bar = harness.rect(harness.find("bar.middle"));
    assert!(
        along(DockEdge::Bottom, bar, body),
        "restored, the middle layer's bar is back on the inner layer's second pane: {bar:?} by {body:?}"
    );
    drop(harness);

    // On the inner layer's first pane, the middle layer's bar dropped after the inner layer's
    // own: the outer layer's bar comes second.
    let (document, layers) = layered_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    settle(&mut harness);
    let inner = harness.rect(harness.find("bar.inner"));
    let grip = grip_of(&harness, "bar.middle");
    harness.drag_and_rest(grip, pos2(inner.right() + 30.0, inner.center().y));
    settle(&mut harness);
    assert!(
        side_by_side(&harness, "bar.inner", "bar.middle"),
        "dropped after the inner layer's bar, the middle layer's bar lands after it"
    );
    assert_eq!(
        middle_bar(&layers.outer).1,
        Some(DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center))
    );

    let saved = snapshots(&layers);
    drop(harness);
    let (document, layers) = layered_dock(Some(saved));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    settle(&mut harness);
    let body = harness.rect(harness.find("content.20"));
    let bar = harness.rect(harness.find("bar.middle"));
    assert!(
        along(DockEdge::Top, bar, body),
        "restored, the middle layer's bar is back on the inner layer's first pane: {bar:?} by {body:?}"
    );
    assert!(
        side_by_side(&harness, "bar.inner", "bar.middle"),
        "restored, the two layers' bars keep the order they were dropped in: inner {:?}, middle {:?}",
        harness.rect(harness.find("bar.inner")),
        harness.rect(harness.find("bar.middle")),
    );
    assert_eq!(
        middle_bar(&layers.outer).1,
        Some(DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center))
    );
}
