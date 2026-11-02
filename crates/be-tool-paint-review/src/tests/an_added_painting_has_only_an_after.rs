use super::*;

#[test]
fn an_added_painting_has_only_an_after() {
    let mut review = Review::with(mixed());
    review
        .test
        .click("paint_review.painting.beui-demo.a_new_page");
    review.test.frame(Vec::new());
    review.deliver();

    assert_eq!(review.shown_name(), "beui-demo.a_new_page");
    assert!(review.pressed("after"));
    review.press(beui::Key::One);
    assert!(
        review.pressed("after"),
        "a painting that is new has nothing before it to show"
    );
    review.test.snapshot("an_added_painting_has_only_an_after");
}
