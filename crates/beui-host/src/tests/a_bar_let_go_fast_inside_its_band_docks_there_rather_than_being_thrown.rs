use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_bar_let_go_fast_inside_its_band_docks_there_rather_than_being_thrown() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let grip = harness.center(harness.find("dock.bar.9.grip"));

    harness.down(grip, PointerButton::Primary);
    let mut at = grip;
    for _ in 0..4 {
        at += vec2(40.0, 4.0);
        harness.hover(at);
    }
    harness.up(at, PointerButton::Primary);
    harness.settle();
    assert!(
        matches!(
            dock.layout.bar_placement(&9),
            Some(DockBarPlacement::Beside(DockEdge::Top, _))
        ),
        "let go while moving fast but still in its band, the bar stays docked there: {:?}",
        dock.layout.bar_placement(&9)
    );
}
