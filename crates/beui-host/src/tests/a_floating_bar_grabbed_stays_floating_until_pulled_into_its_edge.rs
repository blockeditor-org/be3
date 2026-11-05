use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_floating_bar_grabbed_stays_floating_until_pulled_into_its_edge() {
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
    let grabbed = grip.y - body.top();

    harness.down(grip, PointerButton::Primary);
    let along = grip + vec2(-60.0, -grabbed * 0.3);
    harness.hover(grip + vec2(-30.0, 0.0));
    harness.hover(along);
    harness.pause(Duration::from_millis(200));
    assert!(
        harness
            .document()
            .find_test_id("dock.bar.band.top")
            .is_none(),
        "grabbed and nudged a little towards its edge, a floating bar opens no band"
    );
    harness.up(along, PointerButton::Primary);
    harness.settle();
    assert!(
        matches!(
            dock.layout.bar_placement(&9),
            Some(DockBarPlacement::Over(DockEdge::Top, _))
        ),
        "and let go, it floats on: {:?}",
        dock.layout.bar_placement(&9)
    );

    let grip = harness.center(harness.find("dock.bar.9.grip"));
    harness.drag_and_rest(grip, pos2(grip.x, body.top() + 1.0));
    assert!(
        matches!(
            dock.layout.bar_placement(&9),
            Some(DockBarPlacement::Beside(DockEdge::Top, _))
        ),
        "pulled right up into its edge, it docks there: {:?}",
        dock.layout.bar_placement(&9)
    );
}
