use super::*;

#[test]
fn dragging_the_edge_of_the_responsive_screen_resizes_it() {
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.toggle_responsive();
    harness.frame(vec![]);
    let scale = harness.document().screen_scale();
    let shown = harness.shown_screen();

    let right = pos2(shown.right() + 10.0, shown.center().y);
    harness.drag(right, right + vec2(30.0, 0.0));
    harness.frame(vec![]);
    let width = (390.0 + 60.0 / scale).round();
    harness.assert_screen_size(vec2(width, 844.0));

    let shown = harness.shown_screen();
    let scale = harness.document().screen_scale();
    let bottom = pos2(shown.center().x, shown.bottom() + 10.0);
    harness.drag(bottom, bottom - vec2(0.0, 40.0));
    harness.frame(vec![]);
    let height = (844.0 - 40.0 / scale).round();
    harness.assert_screen_size(vec2(width, height));
}
