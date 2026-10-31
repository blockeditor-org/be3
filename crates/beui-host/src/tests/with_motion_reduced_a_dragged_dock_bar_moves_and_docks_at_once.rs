use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};
use crate::reactive::Motion;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn with_motion_reduced_a_dragged_dock_bar_moves_and_docks_at_once() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));
    let bar = harness.rect(harness.find("bar.9"));
    let grip = harness.center(harness.find("dock.bar.9.grip"));
    let grab = grip - bar.min;

    harness.press_at(grip);
    let held = pos2(body.center().x - 80.0, body.center().y + 40.0);
    harness.hover(grip + vec2(0.0, 20.0));
    harness.hover(held);
    harness.frame(Vec::new());
    let lifted = harness.rect(harness.find("bar.9"));
    assert!(
        (lifted.min - (held - grab)).length() < 1.0,
        "the bar is where the pointer holds it at once: {lifted:?}"
    );

    let near = pos2(body.right() - 6.0, body.center().y);
    harness.hover(near);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let band = harness.rect(harness.find("dock.bar.band.right"));
    let docked = harness.rect(harness.find("bar.9"));
    assert!(
        band.contains_rect(docked.shrink(0.5)),
        "the band is open in full at once: {docked:?} in {band:?}"
    );

    harness.release_at(near);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Right, DockAlign::Center))
    );
    let band = harness.rect(harness.find("dock.bar.band.right"));
    let docked = harness.rect(harness.find("bar.9"));
    assert!(
        band.contains_rect(docked.shrink(0.5)),
        "let go, it is in its band at once: {docked:?} in {band:?}"
    );
}
