use super::*;

/// On the web a screen goes above the host's canvas only when an anchored view painted before
/// it lies under it: a pane painted after a popover but beside it stays below, with the host's
/// chrome over it, and so does a screen painted before the popover.
#[test]
fn a_child_over_an_anchored_view_is_lifted_and_a_pane_beside_it_is_not() {
    let rect = |x: f32, y: f32, width: f32, height: f32| {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
    };
    let editor = (rect(0.0, 0.0, 600.0, 800.0), false);
    let popover = (rect(100.0, 100.0, 300.0, 200.0), true);
    let child = (rect(110.0, 110.0, 280.0, 180.0), false);
    let beside = (rect(700.0, 0.0, 500.0, 800.0), false);
    assert_eq!(
        lifted([editor, popover, child, beside]),
        [false, false, true, false]
    );
}
