use super::*;

#[test]
fn a_painting_that_was_never_approved_is_new() {
    let (review, mut editor) = Review::open();
    assert_eq!(review.status(PATH), Some(Status::New));

    editor.click(&entry_id(PATH));
    settled(&mut editor);
    editor.snapshot("a_new_painting_is_shown_for_review");

    editor.click("paint_review.approve");
    editor.run();

    assert_eq!(review.status(PATH), Some(Status::Unchanged));
    assert_eq!(review.approvals(), 1);
    assert_eq!(
        review.approved(PATH).unwrap().data(),
        painting(30).encode().unwrap()
    );
}
