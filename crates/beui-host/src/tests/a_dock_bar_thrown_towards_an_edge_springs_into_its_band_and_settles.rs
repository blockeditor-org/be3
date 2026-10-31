use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_thrown_towards_an_edge_springs_into_its_band_and_settles() {
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
        Some(DockBarPlacement::Beside(DockEdge::Right, DockAlign::Center)),
        "let go in the middle of the pane while moving fast to the right, the bar is thrown to the right edge"
    );
    let released = harness.rect(harness.find("bar.9"));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let flying = harness.rect(harness.find("bar.9"));
    let band = harness.rect(harness.find("dock.bar.band.right"));
    assert!(
        flying.left() > released.left() && flying.left() < band.left() - 20.0,
        "it flies on towards the band rather than jumping there: {released:?} then {flying:?}, band {band:?}"
    );
    let frames = harness.frames_to_settle();
    assert!(frames < 120, "the spring settles: {frames} frames");
    let band = harness.rect(harness.find("dock.bar.band.right"));
    let docked = harness.rect(harness.find("bar.9"));
    assert!(
        band.contains_rect(docked.shrink(0.5)) && docked.height() > docked.width(),
        "and comes to rest standing in the band: {docked:?} in {band:?}"
    );
}
