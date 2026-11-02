use super::*;

#[test]
fn dragging_the_line_moves_the_split() {
    let mut review = Review::open(Some("beui-demo.buttons"));
    review.load(mixed());
    review.deliver();
    review.test.settle();
    let line = review.test.rect_of("paint_review.split");
    let to = beui::pos2(line.center().x - 60.0, line.center().y);
    review.test.drag(line.center(), to);
    review.test.settle();
    let moved = review.test.rect_of("paint_review.split");
    assert!(
        (moved.center().x - to.x).abs() < 2.0,
        "the line follows the pointer: {moved:?}, {to:?}"
    );
    review.test.snapshot("dragging_the_line_moves_the_split");
}
