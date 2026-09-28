use super::*;

#[test]
fn choosing_a_painting_rasters_the_one_it_was_approved_as() {
    let (review, mut editor) = Review::open();
    review.approve(PATH, &painting(90));
    review.write(PATH, &recording(&[30, 200]));
    editor.click("paint_review.refresh");
    editor.run();
    assert_eq!(review.status(PATH), Some(Status::Modified));
    assert_eq!(rasters(&editor), 0);

    editor.click(&entry_id(PATH));
    settled(&mut editor);
    assert_eq!(rasters(&editor), 3);

    editor.click("paint_review.frame.next");
    settled(&mut editor);
    editor.click("paint_review.view.approved");
    settled(&mut editor);
    editor.click("paint_review.view.current");
    settled(&mut editor);
    assert_eq!(rasters(&editor), 3);
}
