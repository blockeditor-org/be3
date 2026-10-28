use super::*;

#[test]
fn a_phone_link_opens_the_named_painting() {
    let mut review = Review::sized(
        Some("text-block.typing_a_paragraph"),
        Vec2::new(390.0, 844.0),
    );
    review.load(mixed());
    review.deliver();
    review.test.settle();
    assert_eq!(review.shown_name(), "text-block.typing_a_paragraph");
    assert!(review.test.shown("paint_review.stage"));
    review
        .test
        .snapshot("a_phone_link_opens_the_named_painting");
}
