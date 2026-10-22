use super::*;

#[test]
fn every_shape_is_drawn_in_the_clean_look() {
    let mut editor = editor(&every_shape(CanvasLook::Clean));
    editor.run();

    editor.snapshot("every_shape_is_drawn_in_the_clean_look");
}
