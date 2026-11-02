use super::*;

#[test]
fn holding_shift_shows_the_changes() {
    let mut review = Review::with(mixed());
    review.deliver();
    review.test.hover("paint_review.painting.beui-demo.buttons");
    review.deliver();

    review.shift(true);
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "beui-demo.buttons: changes, 240 × 160"
    );
    review.test.snapshot("holding_shift_shows_the_changes");
    review.shift(false);
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "beui-demo.buttons: after, 240 × 160"
    );
}
