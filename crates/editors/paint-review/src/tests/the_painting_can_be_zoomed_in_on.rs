use super::*;

#[test]
fn the_painting_can_be_zoomed_in_on() {
    let (_review, mut editor) = Review::open();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    let fitted = shown_zoom(&editor);
    editor.record();

    for _ in 0..4 {
        editor.click("paint_review.zoom.in");
        settled(&mut editor);
    }
    assert!(shown_zoom(&editor) > fitted);
    editor.record();

    editor.click("paint_review.zoom.out");
    settled(&mut editor);
    editor.record();

    editor.click("paint_review.zoom.actual");
    settled(&mut editor);
    assert_eq!(shown_zoom(&editor), 1.0);

    editor.click("paint_review.zoom.fit");
    settled(&mut editor);
    assert_eq!(shown_zoom(&editor), fitted);
    editor.record();

    editor.snapshot("zooming_into_a_painting");
}
