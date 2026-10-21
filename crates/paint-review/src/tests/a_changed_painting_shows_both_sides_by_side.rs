use super::*;

#[test]
fn a_changed_painting_shows_both_sides_by_side() {
    let mut review = Review::with(mixed());
    review.deliver();

    review.test.click("paint_review.mode.side_by_side");
    review.test.frame(Vec::new());

    assert!(review.pressed("side_by_side"));
    review
        .test
        .snapshot("a_changed_painting_shows_both_sides_by_side");
}
