use super::*;

#[test]
fn every_shape_is_drawn_in_the_hand_drawn_look() {
    let mut editor = editor(&every_shape(CanvasLook::HandDrawn));
    editor.run();

    editor.snapshot("every_shape_is_drawn_in_the_hand_drawn_look");
}
