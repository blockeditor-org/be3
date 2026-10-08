use super::*;

#[test]
fn a_secondary_drag_with_the_dock_drag_modifier_moves_the_nearest_split() {
    let SplitDock {
        document,
        dock,
        presses,
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

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    secondary_drag_with(
        &mut harness,
        tab,
        tab - vec2(60.0, 0.0),
        Modifiers::LOGO,
    );
    harness.frame(Vec::new());
    let moved = harness.rect(harness.find("content.1"));
    assert!(
        (after.width() - moved.width() - 60.0).abs() < 1.0,
        "a secondary drag on the tab bar moves the split too, from {} to {} wide",
        after.width(),
        moved.width()
    );
    assert!(
        text_within(harness.document(), dock, "Pop out into a window").is_none(),
        "the tab's menu did not open"
    );
}
