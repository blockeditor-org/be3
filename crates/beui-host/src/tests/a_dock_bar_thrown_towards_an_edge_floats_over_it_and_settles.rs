use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_thrown_towards_an_edge_floats_over_it_and_settles() {
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

    harness.press_at(grip);
    let start = pos2(body.center().x - 120.0, body.center().y);
    harness.hover(grip + vec2(0.0, 20.0));
    harness.hover(start);
    harness.pause(Duration::from_millis(200));
    let mut at = start;
    for _ in 0..4 {
        at += vec2(40.0, 0.0);
        harness.hover(at);
    }
    harness.release_at(at);
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Over(DockEdge::Right, DockAlign::Center)),
        "let go in the middle of the pane while moving fast to the right, the bar is thrown to float over the right edge, never docked"
    );
    let released = harness.rect(harness.find("bar.9"));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let flying = harness.rect(harness.find("bar.9"));
    assert!(
        flying.left() > released.left() && flying.right() < body.right() - 20.0,
        "it flies on towards the edge rather than jumping there: {released:?} then {flying:?} in {body:?}"
    );
    let frames = harness.frames_to_settle();
    assert!(frames < 120, "the spring settles: {frames} frames");
    let floating = harness.rect(harness.find("bar.9"));
    assert!(
        body.contains_rect(floating)
            && body.right() - floating.right() < 16.0
            && floating.height() > floating.width(),
        "and comes to rest standing over the right edge of the pane: {floating:?} in {body:?}"
    );
}
