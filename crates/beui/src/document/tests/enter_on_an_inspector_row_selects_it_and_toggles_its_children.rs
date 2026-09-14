use super::*;

#[test]
fn enter_on_an_inspector_row_selects_it_and_toggles_its_children() {
    let HelloColumn {
        document, padding, ..
    } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());
    harness.toggle_inspector_focus();
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(padding));
    assert_eq!(harness.tree(), ["column", "  frame"]);

    harness.key(Key::Space, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  frame", "    text"]);
}
