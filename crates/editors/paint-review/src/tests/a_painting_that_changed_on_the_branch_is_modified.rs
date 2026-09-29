use super::*;

#[test]
fn a_painting_that_changed_on_the_branch_is_modified() {
    let (review, mut editor) = Review::open();
    editor.click(&entry_id(PATH));
    settled(&mut editor);
    editor.click("paint_review.approve");
    editor.run();
    let snapshot = review.reference(PATH);

    review.write(PATH, &painting(200));
    editor.click("paint_review.refresh");
    settled(&mut editor);
    assert_eq!(review.status(PATH), Some(Status::Modified));

    editor.click("paint_review.view.approved");
    settled(&mut editor);
    editor.click("paint_review.view.current");
    settled(&mut editor);

    editor.click("paint_review.approve");
    settled(&mut editor);
    assert_eq!(review.status(PATH), Some(Status::Unchanged));
    assert_eq!(review.approvals(), 1);
    assert_eq!(review.reference(PATH), snapshot);
    assert_eq!(
        review.approved(PATH).unwrap().data(),
        painting(200).encode().unwrap()
    );
}
