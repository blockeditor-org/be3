use super::*;

#[test]
fn a_painting_that_vanished_is_removed() {
    let (review, mut editor) = Review::open();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    editor.click("paint_review.approve");
    editor.run();

    review.remove(PATH);
    editor.click("paint_review.refresh");
    editor.run();
    assert_eq!(review.status(PATH), Some(Status::Removed));
    assert!(review.approved(PATH).is_some());

    editor.click("paint_review.unapprove");
    editor.run();
    assert_eq!(review.status(PATH), None);
    assert_eq!(review.approvals(), 0);
}
