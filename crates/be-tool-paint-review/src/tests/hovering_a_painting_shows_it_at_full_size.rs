use super::*;

#[test]
fn hovering_a_painting_shows_it_at_full_size() {
    let mut review = Review::with(mixed());
    review.deliver();

    review.test.hover("paint_review.painting.beui-demo.buttons");
    assert!(
        review
            .requested()
            .contains(&"after-beui-demo.buttons".to_owned()),
        "hovering fetches the full image at once"
    );
    assert!(review.test.shows("paint_review.preview"));
    review.deliver();
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "beui-demo.buttons: after, 240 × 160"
    );
    let preview = review.test.rect_of("paint_review.preview");
    assert!(preview.width() > WIDTH as f32 && preview.height() > HEIGHT as f32);
    assert!((preview.center().x - WIDE.x / 2.0).abs() < 1.0);
    review
        .test
        .snapshot("hovering_a_painting_shows_it_at_full_size");

    review
        .test
        .hover("paint_review.painting.text-block.typing_a_paragraph");
    review.deliver();
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "text-block.typing_a_paragraph: after, 240 × 160",
        "moving to the next painting, even under the preview, shows that one"
    );
    review.test.leave();
    review.test.frame(Vec::new());
    assert!(!review.test.shows("paint_review.preview"));
}
