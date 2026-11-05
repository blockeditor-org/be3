use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_bar_carried_along_its_band_stays_level_with_it_while_held_and_let_go() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let bar = harness.rect(harness.find("bar.9"));
    let grip = harness.center(harness.find("dock.bar.9.grip"));

    harness.down(grip, PointerButton::Primary);
    for step in 1..=8 {
        harness.hover(grip + vec2(step as f32 * -15.0, 12.0));
        let held = harness.rect(harness.find("bar.9"));
        assert_eq!(
            held.top(),
            bar.top(),
            "step {step}: pulled a little off its band, the bar slides along it and never follows the pointer across"
        );
    }
    harness.up(grip + vec2(-120.0, 12.0), PointerButton::Primary);
    for frame in 0..40 {
        harness.frame(Vec::new());
        let settling = harness.rect(harness.find("bar.9"));
        assert_eq!(
            settling.top(),
            bar.top(),
            "frame {frame}: let go, it settles along the band without dipping out of it"
        );
    }
    assert!(
        matches!(
            dock.layout.bar_placement(&9),
            Some(DockBarPlacement::Beside(DockEdge::Top, _))
        ),
        "it stays docked in the band: {:?}",
        dock.layout.bar_placement(&9)
    );
}
