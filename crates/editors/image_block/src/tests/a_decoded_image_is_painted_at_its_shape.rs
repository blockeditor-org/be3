use super::*;

#[test]
fn a_decoded_image_is_painted_at_its_shape() {
    let mut editor = editor(png(8, 4));

    editor.run();

    let canvas = editor.rect();
    let rect = editor.rect_of("image.picture");
    assert!(rect.is_positive());
    assert!(
        rect.width() > canvas.width() / 2.0,
        "the image must fill the space the camera gave it"
    );
    let header = editor.content::<ImageContent>(None).header().clone();
    assert_eq!(
        (header.media_type.as_str(), header.width, header.height),
        ("image/png", 8, 4)
    );
    editor.snapshot("a_decoded_image_is_painted_at_its_shape");
}
