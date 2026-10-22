use super::*;

#[test]
fn the_painting_named_in_the_address_is_shown_first() {
    let mut review = Review::open(Some("text-block.typing_a_paragraph"));
    review.load(mixed());

    assert_eq!(review.shown_name(), "text-block.typing_a_paragraph");

    let mut review = Review::open(Some("no.such_painting"));
    review.load(mixed());
    assert_eq!(
        review.shown_name(),
        "beui-demo.buttons",
        "a name the pull request did not change opens on the first painting listed"
    );
}
