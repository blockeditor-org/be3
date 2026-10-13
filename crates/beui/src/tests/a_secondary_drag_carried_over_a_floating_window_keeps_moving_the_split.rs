use super::*;

#[test]
fn a_secondary_drag_carried_over_a_floating_window_keeps_moving_the_split() {
    let SplitDock {
        document,
        dock,
        presses,
    } = split_dock(Some(Modifiers::LOGO));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    floated_window(&mut harness, dock);
    harness.frame(Vec::new());
    let window = harness.rect(harness.find("content.2"));
    let before = harness.rect(harness.find("content.1"));

    let from = pos2(window.min.x - 30.0, window.center().y);
    let to = pos2(window.min.x + 30.0, window.center().y);
    assert!(
        before.contains(from) && !window.contains(from) && window.contains(to),
        "the drag starts beside the window and ends over it"
    );
    secondary_drag_with(&mut harness, from, to, Modifiers::LOGO);
    harness.frame(Vec::new());

    let after = harness.rect(harness.find("content.1"));
    assert!(
        (after.width() - before.width() - (to.x - from.x)).abs() < 1.0,
        "the split followed the pointer onto the window, from {} to {} wide",
        before.width(),
        after.width()
    );
    assert_eq!(presses.get(), 0);
}
