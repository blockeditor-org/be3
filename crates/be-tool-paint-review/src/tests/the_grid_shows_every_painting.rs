use super::*;

#[test]
fn the_grid_shows_every_painting() {
    let mut review = Review::with(mixed());
    review.deliver();

    for name in [
        "beui-demo.a_new_page",
        "checklist.an_old_list",
        "beui-demo.buttons",
        "text-block.typing_a_paragraph",
        "map.broken",
    ] {
        assert!(review.test.shows(&format!("paint_review.painting.{name}")));
    }
    let x = |name: &str| {
        review
            .test
            .rect_of(&format!("paint_review.painting.{name}"))
            .min
            .x
    };
    assert!(
        x("beui-demo.a_new_page") < x("checklist.an_old_list"),
        "the paintings stay in the order of the list, largest changes first"
    );
    assert!(!review.test.shows("paint_review.preview"));
    review.test.snapshot("the_grid_shows_every_painting");
}
