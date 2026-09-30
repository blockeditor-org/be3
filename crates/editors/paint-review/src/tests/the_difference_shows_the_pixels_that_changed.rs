use super::*;

#[test]
fn the_difference_shows_the_pixels_that_changed() {
    let (review, mut editor) = Review::open();
    review.approve(PATH, &marked(30, 3.0));
    review.write(PATH, &marked(30, 12.0));
    editor.click("paint_review.refresh");
    editor.run();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    editor.record();

    editor.click("paint_review.view.difference");
    settled(&mut editor);
    let rastered = rasters(&editor);
    editor.record();

    editor.click("paint_review.view.side_by_side");
    settled(&mut editor);
    editor.record();
    assert_eq!(rasters(&editor), rastered);

    editor.click("paint_review.view.approved");
    settled(&mut editor);
    editor.record();
    assert_eq!(rasters(&editor), rastered);

    editor.snapshot("comparing_a_painting_with_the_one_it_was_approved_as");
}
