use super::*;

#[test]
fn the_left_and_right_arrows_collapse_and_expand_an_inspector_row() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());
    harness.toggle_inspector_focus();
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::NONE);

    assert_eq!(harness.tree(), ["column", "  frame"]);
    assert_eq!(harness.focused_row_index(), Some(1));

    harness.key(Key::ArrowLeft, Modifiers::NONE);

    assert_eq!(
        harness.focused_row_index(),
        Some(0),
        "left reaches the parent"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);

    assert_eq!(harness.tree(), ["column", "  frame", "    text"]);
    assert_eq!(harness.focused_row_index(), Some(1));

    harness.key(Key::ArrowRight, Modifiers::NONE);

    assert_eq!(
        harness.focused_row_index(),
        Some(2),
        "right reaches the child"
    );
}
