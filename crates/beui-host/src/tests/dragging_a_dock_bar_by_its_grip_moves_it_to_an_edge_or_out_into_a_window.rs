use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn dragging_a_dock_bar_by_its_grip_moves_it_to_an_edge_or_out_into_a_window() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));

    let grip = harness.center(harness.find("dock.bar.9.grip"));
    harness.drag(grip, pos2(body.right() - 4.0, body.center().y));
    harness.frame(Vec::new());
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Right)),
        "dropped right at the edge, the bar docks beside the body"
    );

    let grip = harness.center(harness.find("dock.bar.9.grip"));
    let band = (body.width().min(body.height()) * 0.12).clamp(24.0, 56.0);
    harness.drag(grip, pos2(body.center().x, body.bottom() - band * 1.5));
    harness.frame(Vec::new());
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center)),
        "dropped a little in from the edge, the bar floats over the body"
    );

    let side = harness.rect(harness.find("content.2"));
    let grip = harness.center(harness.find("dock.bar.9.grip"));
    harness.drag(grip, side.center());
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let Some(DockBarPlacement::Window { edge, .. }) = dock.layout.bar_placement(&9) else {
        panic!("dropped away from its pane, the bar pops out into a window");
    };
    assert_eq!(
        edge,
        DockEdge::Bottom,
        "the window keeps the edge the bar came from"
    );
    let bar = harness.rect(harness.find("bar.9"));
    assert!(
        side.contains(bar.center()),
        "the window opens where the bar was dropped: {bar:?} in {side:?}, {:?}",
        dock.layout.bar_placement(&9)
    );
    assert!(bar.width() > bar.height(), "it keeps its orientation");

    let grip = harness.center(harness.find("dock.bar.9.grip"));
    harness.drag(grip, pos2(body.left() + 4.0, body.center().y));
    harness.frame(Vec::new());
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Left)),
        "a popped out bar docks again when dragged back over its pane"
    );
}
