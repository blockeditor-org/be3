use super::*;

#[test]
fn a_narrow_page_stacks_each_control_under_its_label() {
    let mut editor = editor(vec![touchpad()]);
    editor.set_size(block_editor_beui::beui::Vec2::new(360.0, 900.0));
    editor.run();
    editor.click("input.layout");
    editor.run();
    editor.type_text("de");
    editor.run();
    assert!(editor.shown("input.layout.menu"));
    editor.snapshot("a_narrow_page_stacks_each_control_under_its_label");
}
