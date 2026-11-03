use super::*;

#[test]
fn an_added_painting_has_nothing_before_the_line() {
    let mut review = Review::open(Some("beui-demo.a_new_page"));
    review.load(mixed());
    review.deliver();
    review.test.settle();

    assert_eq!(
        review.text("paint_review.showing"),
        "Showing before and after"
    );
    assert!(review.test.shows("paint_review.split"));
    review
        .test
        .snapshot("an_added_painting_has_nothing_before_the_line");

    review.press(beui::Key::J);
    review.deliver();
    review.test.settle();
    assert_eq!(review.shown_name(), "checklist.an_old_list");
    assert!(review.test.shows("paint_review.split"));
    review
        .test
        .snapshot("a_removed_painting_has_nothing_after_the_line");
}
