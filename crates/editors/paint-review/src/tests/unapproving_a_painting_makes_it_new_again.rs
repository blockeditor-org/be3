use super::*;

#[test]
fn unapproving_a_painting_makes_it_new_again() {
    let (review, mut editor) = Review::open();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    editor.click("paint_review.approve");
    editor.run();
    assert_eq!(review.status(PATH), Some(Status::Unchanged));
    let snapshot = review.reference(PATH).unwrap();

    editor.click("paint_review.unapprove");
    editor.run();

    assert_eq!(review.status(PATH), Some(Status::New));
    assert_eq!(review.approvals(), 0);
    assert!(review.orphaned(snapshot));

    editor.click("paint_review.approve");
    editor.run();
    assert_eq!(review.status(PATH), Some(Status::Unchanged));
    assert_eq!(
        review.approved(PATH).unwrap().data(),
        painting(30).encode().unwrap()
    );
}
