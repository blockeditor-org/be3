use super::*;

#[test]
fn a_changed_painting_shows_what_it_became() {
    let mut review = Review::with(mixed());
    review.deliver();

    assert_eq!(review.shown_name(), "beui-demo.buttons");
    assert!(
        review.pressed("after"),
        "a changed painting opens on what it became"
    );
    review
        .test
        .snapshot("a_changed_painting_shows_what_it_became");
}
