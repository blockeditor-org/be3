use super::*;

#[test]
fn a_bullet_a_peer_makes_a_task_draws_a_checkbox() {
    let mut editor = editor("# T\n\n- item");
    editor.run();
    peer_insert(&mut editor, 7, "[ ] ");
    editor.run();

    assert_eq!(text(&editor), "# T\n\n- [ ] item");
    assert!(editor.shown("text.checkbox.2"));
    editor.snapshot("a_bullet_a_peer_makes_a_task_draws_a_checkbox");
}
