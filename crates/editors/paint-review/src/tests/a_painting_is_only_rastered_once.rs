use super::*;

#[test]
fn a_painting_is_only_rastered_once() {
    let (review, mut editor) = Review::open();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    assert_eq!(rasters(&editor), 1);

    editor.click("paint_review.approve");
    editor.run();
    assert_eq!(rasters(&editor), 1);

    review.write(PATH, &painting(200));
    editor.click("paint_review.refresh");
    settled(&mut editor);
    assert_eq!(rasters(&editor), 2);

    editor.click("paint_review.view.approved");
    settled(&mut editor);
    editor.click("paint_review.view.current");
    settled(&mut editor);
    editor.click("paint_review.view.approved");
    settled(&mut editor);
    assert_eq!(rasters(&editor), 2);
}
