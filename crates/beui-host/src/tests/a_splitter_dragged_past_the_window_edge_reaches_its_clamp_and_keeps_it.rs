use super::*;
use crate::WindowId;

#[test]
fn a_splitter_dragged_past_the_window_edge_reaches_its_clamp_and_keeps_it() {
    let _root = crate::reactive::RootScope::new();
    let SplitDock { document, .. } = split_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let width = |harness: &Harness| harness.rect(harness.find("content.1")).width();
    let left = harness.rect(harness.find("content.1"));
    let right = harness.rect(harness.find("content.2"));
    let gap = pos2((left.right() + right.left()) / 2.0, right.center().y);

    // Dragged to the window's left edge, it stops at its clamp.
    press_in(&mut harness, WindowId::MAIN, gap, true);
    let edge = pos2(1.0, gap.y);
    harness.frame(vec![Input::PointerMoved(edge)]);
    harness.frame(Vec::new());
    let clamped = width(&harness);
    assert!(
        clamped < left.width() - 100.0,
        "dragged to the edge, the left pane narrows: {} to {clamped}",
        left.width()
    );
    press_in(&mut harness, WindowId::MAIN, edge, false);
    harness.settle();

    // Back, then past the edge and outside the window: it follows there too.
    let gap = pos2(left.right() + (right.left() - left.right()) / 2.0, gap.y);
    let gap = pos2(gap.x - (left.width() - clamped), gap.y);
    press_in(&mut harness, WindowId::MAIN, gap, true);
    harness.frame(vec![Input::PointerMoved(gap + vec2(200.0, 0.0))]);
    harness.frame(Vec::new());
    assert!(
        (width(&harness) - clamped - 200.0).abs() < 1.0,
        "the splitter is carried again"
    );
    harness.frame(vec![Input::PointerLeft]);
    let beyond = pos2(-300.0, gap.y);
    harness.frame(vec![Input::PointerMoved(beyond)]);
    harness.frame(Vec::new());
    assert_eq!(
        width(&harness),
        clamped,
        "past the window's edge the splitter reaches its clamp"
    );
    press_in(&mut harness, WindowId::MAIN, beyond, false);
    harness.settle();
    assert_eq!(
        width(&harness),
        clamped,
        "and a release outside the window keeps it there"
    );
}
