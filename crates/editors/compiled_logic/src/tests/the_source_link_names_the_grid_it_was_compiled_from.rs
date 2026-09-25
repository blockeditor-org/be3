use super::*;

#[test]
fn the_source_link_names_the_grid_it_was_compiled_from() {
    let mut editor = editor_on(Uuid::new_v4(), Some("Half adder"));
    editor.run();

    assert_eq!(editor.label("compiled-logic.source"), "Half adder");
}
