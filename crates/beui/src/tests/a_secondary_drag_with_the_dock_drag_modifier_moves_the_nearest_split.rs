use super::*;

#[test]
fn a_secondary_drag_with_the_dock_drag_modifier_moves_the_nearest_split() {
    let SplitDock {
        document, presses, ..
    } = split_dock(Some(Modifiers::LOGO));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let before = harness.rect(harness.find("content.1"));

    let from = before.center();
    secondary_drag_with(
        &mut harness,
        from,
        from + vec2(120.0, 40.0),
        Modifiers::LOGO,
    );
    harness.frame(Vec::new());

    let after = harness.rect(harness.find("content.1"));
    assert!(
        (after.width() - before.width() - 120.0).abs() < 1.0,
        "the split beside the pane follows the pointer across, from {} to {} wide",
        before.width(),
        after.width()
    );
    assert_eq!(
        presses.get(),
        0,
        "the secondary press that took the split never reached the content"
    );
}
