use super::*;

#[test]
fn a_decoded_image_is_painted_at_its_shape() {
    let (mut editor, _) = editor(png(8, 4));

    editor.run();

    let rect = editor.rect_of("image.picture");
    assert!(rect.is_positive());
    editor.snapshot("a_decoded_image_is_painted_at_its_shape");
}
