use super::*;

#[test]
fn an_image_that_has_not_arrived_shows_it_is_loading() {
    let mut review = Review::with(mixed());

    assert!(review.test.shown("paint_review.image.loading"));
    review
        .test
        .snapshot("an_image_that_has_not_arrived_shows_it_is_loading");

    review.deliver();
    assert!(!review.test.shown("paint_review.image.loading"));
}
