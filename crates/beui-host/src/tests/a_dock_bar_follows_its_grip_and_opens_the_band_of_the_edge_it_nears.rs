use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_follows_its_grip_and_opens_the_band_of_the_edge_it_nears() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));
    let bar = harness.rect(harness.find("bar.9"));
    let grip = harness.center(harness.find("dock.bar.9.grip"));
    let grab = grip - bar.min;

    harness.down(grip, PointerButton::Primary);
    let held = pos2(body.center().x - 80.0, body.center().y + 40.0);
    harness.hover(grip + vec2(0.0, 20.0));
    harness.hover(held);
    for _ in 0..30 {
        harness.frame(Vec::new());
    }
    let lifted = harness.rect(harness.find("bar.9"));
    assert!(
        (lifted.min - (held - grab)).length() < 1.0,
        "the bar follows the pointer, held where it was grabbed: {lifted:?} under {held:?}"
    );
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Over(DockEdge::Top, DockAlign::Center)),
        "nothing moves for good until the bar is let go"
    );

    let near = pos2(body.right() - 6.0, body.center().y);
    harness.hover(near);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let band = harness.rect(harness.find("dock.bar.band.right"));
    let docked = harness.rect(harness.find("bar.9"));
    assert!(
        docked.height() > docked.width(),
        "nearing the right edge, the bar stands up along it: {docked:?}"
    );
    assert!(
        band.contains_rect(docked.shrink(0.5)),
        "and sits in the band that opens there: {docked:?} in {band:?}"
    );
    let narrowed = harness.rect(harness.find("content.1"));
    assert!(
        narrowed.right() <= band.left() + 0.5 && narrowed.right() < body.right() - 4.0,
        "the body gives the band its room: {narrowed:?} beside {band:?}"
    );

    harness.hover(held);
    for _ in 0..30 {
        harness.frame(Vec::new());
    }
    assert!(
        harness
            .document()
            .find_test_id("dock.bar.band.right")
            .is_none(),
        "pulled away, the band closes"
    );
    let floating = harness.rect(harness.find("bar.9"));
    assert!(
        floating.width() > floating.height(),
        "the bar lies along the top edge it came from again: {floating:?}"
    );

    let below_its_place = pos2(body.center().x, body.top() + body.height() * 0.35);
    harness.hover(below_its_place);
    harness.pause(Duration::from_millis(200));
    harness.up(below_its_place, PointerButton::Primary);
    harness.settle();
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Over(DockEdge::Top, DockAlign::Center)),
        "let go at rest away from every edge but nearest the middle of the top, the bar floats back over it"
    );
    let home = harness.rect(harness.find("bar.9"));
    assert!(
        (home.min - bar.min).length() < 1.0,
        "and springs back into its place: {home:?}, was {bar:?}"
    );
}
