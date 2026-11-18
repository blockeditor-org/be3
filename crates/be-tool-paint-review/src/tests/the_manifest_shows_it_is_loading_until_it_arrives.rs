use super::*;

#[test]
fn the_manifest_shows_it_is_loading_until_it_arrives() {
    let mut review = Review::open(None);

    assert!(review.test.shown("paint_review.loading"));
    review
        .test
        .snapshot("the_manifest_shows_it_is_loading_until_it_arrives");

    review.load(mixed());
    assert!(!review.test.shown("paint_review.loading"));
    assert!(review.test.shown("paint_review.painting.beui-demo.buttons"));
}
