use super::*;

#[test]
fn a_phone_shows_the_grid() {
    let mut review = Review::sized(None, Vec2::new(390.0, 844.0));
    review.load(mixed());
    review.deliver();
    review.test.settle();
    review.test.snapshot("a_phone_shows_the_grid");
    review.test.click("paint_review.painting.beui-demo.buttons");
    review.deliver();
    review.test.settle();
    assert!(review.test.shows("paint_review.stage"));
    review.test.snapshot("a_phone_compares_a_painting");
}
