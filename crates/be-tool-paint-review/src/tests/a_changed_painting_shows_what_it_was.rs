use super::*;

#[test]
fn a_changed_painting_shows_what_it_was() {
    let mut review = Review::with(mixed());
    review.deliver();

    review.test.click("paint_review.mode.before");
    review.test.frame(Vec::new());

    assert!(review.pressed("before"));
    review.test.snapshot("a_changed_painting_shows_what_it_was");
}
