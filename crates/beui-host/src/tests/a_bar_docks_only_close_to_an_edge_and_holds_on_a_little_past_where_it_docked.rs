use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_bar_docks_only_close_to_an_edge_and_holds_on_a_little_past_where_it_docked() {
    let _root = crate::reactive::RootScope::new();
    let (document, _dock) = bar_dock(
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));
    let bar = harness.rect(harness.find("bar.9"));
    let thickness = bar.width().min(bar.height());
    let grip = harness.center(harness.find("dock.bar.9.grip"));
    let docked = |harness: &Harness| {
        harness
            .document()
            .find_test_id("dock.bar.band.right")
            .is_some()
    };
    let from_right = |by: f32| pos2(body.right() - by, body.center().y);

    harness.down(grip, PointerButton::Primary);
    harness.hover(grip + vec2(0.0, 20.0));
    harness.hover(body.center());
    harness.hover(from_right(thickness * 0.75));
    harness.pause(Duration::from_millis(200));
    assert!(
        !docked(&harness),
        "three quarters of a bar from the right edge, the bar opens no band there"
    );
    harness.hover(from_right(thickness * 0.25));
    harness.pause(Duration::from_millis(200));
    assert!(
        docked(&harness),
        "a quarter of a bar from it, the band opens"
    );
    harness.hover(from_right(thickness * 1.25));
    harness.pause(Duration::from_millis(200));
    assert!(
        docked(&harness),
        "pulled back out past where it docked, the band holds on to it"
    );
    harness.hover(from_right(thickness * 2.0));
    for _ in 0..30 {
        harness.frame(Vec::new());
    }
    assert!(!docked(&harness), "two bars out, it lets go");
    harness.up(from_right(thickness * 2.0), PointerButton::Primary);
    harness.settle();
}
