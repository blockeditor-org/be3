use super::*;

/// A bullet's marker becomes a checkbox when the line is made a task: the marker is built anew
/// when its kind changes, not kept from the bullet.
#[test]
fn a_bullet_typed_into_a_task_draws_a_checkbox() {
    let mut editor = editor("# T\n\n- ");
    editor.click("text.surface");
    editor.key(Key::End, Modifiers::CTRL);
    editor.type_text("[ ] milk");

    assert_eq!(text(&editor), "# T\n\n- [ ] milk");
    assert!(editor.shown("text.checkbox.2"));
    editor.snapshot("a_bullet_typed_into_a_task_draws_a_checkbox");
}
