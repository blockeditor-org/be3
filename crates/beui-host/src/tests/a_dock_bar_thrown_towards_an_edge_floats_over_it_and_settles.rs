use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

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

    harness.down(grip, PointerButton::Primary);
    let start = pos2(body.center().x - 120.0, body.center().y);
    harness.hover(grip + vec2(0.0, 20.0));
    harness.hover(start);
    harness.pause(Duration::from_millis(200));
    let mut at = start;
    for _ in 0..4 {
        at += vec2(40.0, 0.0);
        harness.hover(at);
    }
    harness.up(at, PointerButton::Primary);
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
    let mut lefts = vec![flying.left()];
    loop {
        let output = harness.frame(Vec::new());
        lefts.push(harness.rect(harness.find("bar.9")).left());
        if !output.repaint && output.repaint_after > Duration::ZERO {
            break;
        }
        assert!(lefts.len() < 120, "the spring settles");
    }
    let floating = harness.rect(harness.find("bar.9"));
    let rest = floating.left();
    let passes = lefts
        .windows(2)
        .filter(|pair| (pair[0] - rest) * (pair[1] - rest) < 0.0)
        .count();
    let beyond = lefts.iter().fold(0.0_f32, |most, left| most.max(left - rest));
    assert!(
        passes == 1 && beyond > 1.0,
        "it swings past where it lands once, by {beyond}, and lands there on the way back \
         without passing it again ({passes} passes): {lefts:?}"
    );
    assert!(
        body.contains_rect(floating)
            && body.right() - floating.right() < 16.0
            && floating.height() > floating.width(),
        "and comes to rest standing over the right edge of the pane: {floating:?} in {body:?}"
    );
}
