use super::*;

#[test]
fn editing_gives_the_slide_the_whole_stage() {
    let (mut test, editor) = editor(3);
    test.run();

    assert_eq!(test.rect_of("presentation.stage"), editor.content_rect());

    editor.host().set_presenting(true);
    test.run();

    let stage = editor.content_rect();
    let slide = test.rect_of("presentation.stage");
    assert!(
        slide.height() < stage.height(),
        "presenting should letterbox the slide inside the stage"
    );
    assert!((slide.width() / slide.height() - 16.0 / 9.0).abs() < 0.01);
}
