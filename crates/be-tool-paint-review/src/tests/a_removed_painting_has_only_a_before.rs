use super::*;

#[test]
fn a_removed_painting_has_only_a_before() {
    let mut review = Review::with(mixed());
    review
        .test
        .click("paint_review.painting.checklist.an_old_list");
    review.test.frame(Vec::new());
    review.deliver();

    assert_eq!(review.shown_name(), "checklist.an_old_list");
    assert!(
        review.pressed("before"),
        "a removed painting has only what it was"
    );
    review.press(beui::Key::Space);
    assert!(
        review.pressed("before"),
        "there is nothing to flip a removed painting to"
    );
    review.test.snapshot("a_removed_painting_has_only_a_before");
}
