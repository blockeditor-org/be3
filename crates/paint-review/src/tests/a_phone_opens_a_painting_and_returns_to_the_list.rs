use super::*;

#[test]
fn a_phone_opens_a_painting_and_returns_to_the_list() {
    let mut review = Review::sized(None, Vec2::new(390.0, 844.0));
    review.load(mixed());
    review.test.settle();
    review.test.snapshot("a_phone_lists_the_paintings");
    review.test.click("paint_review.painting.beui-demo.buttons");
    review.deliver();
    review.test.settle();
    assert!(review.test.shown("paint_review.stage"));
    review.test.snapshot("a_phone_shows_the_selected_painting");
    review.test.click("dock.back");
    review.test.settle();
    assert!(review.test.shown("paint_review.summary"));
    assert!(!review.test.shown("paint_review.stage"));
}
