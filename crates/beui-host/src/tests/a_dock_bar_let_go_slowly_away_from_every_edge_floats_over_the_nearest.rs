use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_let_go_slowly_away_from_every_edge_floats_over_the_nearest() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));
    let grip = harness.center(harness.find("dock.bar.9.grip"));

    let near_left = pos2(
        body.left() + body.width() * 0.3,
        body.bottom() - body.height() * 0.4,
    );
    harness.drag_and_rest(grip, near_left);
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Over(DockEdge::Left, DockAlign::Center)),
        "let go at rest nearer the left edge than any other, the bar floats over it there"
    );
}
