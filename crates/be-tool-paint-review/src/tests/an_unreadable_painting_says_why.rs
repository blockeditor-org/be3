use super::*;

#[test]
fn an_unreadable_painting_says_why() {
    let mut review = Review::with(mixed());
    review.deliver();
    review.test.click("paint_review.painting.map.broken");
    review.test.frame(Vec::new());

    assert_eq!(review.shown_name(), "map.broken");
    assert!(review.test.shows("paint_review.detail"));
    assert!(review.test.shows("paint_review.image.none"));
    assert!(
        !review
            .requested()
            .iter()
            .any(|hash| hash.contains("map.broken")),
        "an unreadable painting has no image to fetch"
    );
    review.test.snapshot("an_unreadable_painting_says_why");
}
