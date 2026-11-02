use super::*;

#[test]
fn a_changed_painting_shows_its_changes_in_magenta() {
    let mut review = Review::with(mixed());
    review.deliver();

    review.test.click("paint_review.mode.changes");
    review.test.frame(Vec::new());

    assert!(review.pressed("changes"));
    review
        .test
        .snapshot("a_changed_painting_shows_its_changes_in_magenta");
}
