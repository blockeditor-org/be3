use super::*;

#[test]
fn a_release_that_moves_the_simulated_screens_edge_resizes_it_to_it() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.toggle_responsive();
    harness.frame(vec![]);
    let scale = harness.document().screen_scale();
    let shown = harness.shown_screen();
    let right = pos2(shown.right() + 10.0, shown.center().y);
    let to = right + vec2(30.0, 0.0);
    let button = |pos, pressed| Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    };

    harness.frame(vec![Input::PointerMoved(right)]);
    harness.frame(vec![button(right, true)]);
    harness.frame(vec![Input::PointerMoved(right + vec2(15.0, 0.0))]);
    harness.frame(vec![Input::PointerMoved(to), button(to, false)]);
    harness.frame(vec![]);

    harness.assert_screen_size(vec2((390.0 + 60.0 / scale).round(), 844.0));
}
