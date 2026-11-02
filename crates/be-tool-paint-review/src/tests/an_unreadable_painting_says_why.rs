use super::*;

#[test]
fn an_unreadable_painting_says_why() {
    let mut review = Review::with(mixed());
    review.test.click("paint_review.painting.map.broken");
    review.test.frame(Vec::new());

    assert_eq!(review.shown_name(), "map.broken");
    assert!(review.test.shows("paint_review.detail"));
    assert!(review.test.shows("paint_review.image.none"));
    assert_eq!(
        review.requested().len(),
        3,
        "only the first painting's images were asked for"
    );
    review.test.snapshot("an_unreadable_painting_says_why");
}
