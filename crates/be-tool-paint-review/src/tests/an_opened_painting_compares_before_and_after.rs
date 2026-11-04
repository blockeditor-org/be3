use super::*;

#[test]
fn an_opened_painting_compares_before_and_after() {
    let mut review = Review::with(mixed());
    review.test.click("paint_review.painting.beui-demo.buttons");
    review.deliver();

    assert_eq!(review.shown_name(), "beui-demo.buttons");
    assert_eq!(review.text("paint_review.position"), "3 of 5");
    assert_eq!(
        review.text("paint_review.showing"),
        "Showing before and after"
    );
    assert!(review.test.shows("paint_review.split"));
    review
        .test
        .snapshot("an_opened_painting_compares_before_and_after");

    review.hold(beui::Key::Space, true);
    assert_eq!(review.text("paint_review.showing"), "Showing before");
    review.hold(beui::Key::Space, false);
    review.shift(true);
    assert_eq!(review.text("paint_review.showing"), "Showing changes");
    review.test.snapshot("an_opened_painting_shows_its_changes");
    review.shift(false);

    review.test.click("paint_review.back");
    assert!(review.test.shows("paint_review.grid"));
}
