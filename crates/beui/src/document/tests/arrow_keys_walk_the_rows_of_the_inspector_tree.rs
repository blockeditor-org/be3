use super::*;

#[test]
fn arrow_keys_walk_the_rows_of_the_inspector_tree() {
    let HelloColumn {
        document,
        padding,
        text,
    } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());
    harness.toggle_inspector_focus();

    harness.key(Key::ArrowDown, Modifiers::NONE);

    assert_eq!(harness.focused_row_index(), Some(1));
    assert_eq!(harness.inspector().state.selected.get(), Some(padding));

    harness.key(Key::ArrowDown, Modifiers::NONE);

    assert_eq!(harness.focused_row_index(), Some(2));
    assert_eq!(harness.inspector().state.selected.get(), Some(text));

    harness.key(Key::ArrowDown, Modifiers::NONE);

    assert_eq!(
        harness.focused_row_index(),
        Some(2),
        "the last row stays put"
    );

    harness.key(Key::Home, Modifiers::NONE);

    assert_eq!(harness.focused_row_index(), Some(0));

    harness.key(Key::End, Modifiers::NONE);

    assert_eq!(harness.focused_row_index(), Some(2));

    harness.key(Key::ArrowUp, Modifiers::NONE);

    assert_eq!(harness.focused_row_index(), Some(1));
    assert_eq!(harness.inspector().state.selected.get(), Some(padding));
}
