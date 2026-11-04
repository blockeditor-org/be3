use super::*;

#[test]
fn moving_between_paintings_keeps_the_preview() {
    let mut review = Review::with(mixed());
    review.deliver();
    let first = review
        .test
        .rect_of("paint_review.painting.beui-demo.a_new_page");
    let second = review
        .test
        .rect_of("paint_review.painting.checklist.an_old_list");
    assert_eq!(
        first.max.x, second.min.x,
        "the paintings touch, leaving no gap"
    );

    let y = first.center().y;
    let mut x = first.center().x;
    while x < second.center().x {
        review
            .test
            .frame(vec![Input::PointerMoved(beui::pos2(x, y))]);
        review.test.frame(Vec::new());
        assert!(
            review.test.shows("paint_review.preview"),
            "the preview stays up at x {x}"
        );
        x += 4.0;
    }
    review.deliver();
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "checklist.an_old_list: removed"
    );
}
