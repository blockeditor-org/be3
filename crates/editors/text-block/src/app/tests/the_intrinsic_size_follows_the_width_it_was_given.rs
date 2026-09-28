use super::editor;
use beui::Vec2;

#[test]
fn the_intrinsic_size_follows_the_width_it_was_given() {
    let mut editor = editor("one\ntwo\nthree\n");

    editor.resize(Vec2::new(280.0, 0.0));
    editor.run();
    let narrow = editor.intrinsic_size().expect("the document is loaded");
    editor.resize(Vec2::new(640.0, 0.0));
    editor.run();
    let wide = editor.intrinsic_size().expect("the document is loaded");

    assert_eq!(narrow.x, 280.0);
    assert_eq!(wide.x, 640.0);
}
