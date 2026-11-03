use super::*;

#[test]
fn holding_space_shows_what_the_paintings_were() {
    let mut review = Review::with(mixed());
    review.deliver();
    review
        .test
        .hover("paint_review.painting.beui-demo.a_new_page");
    review.deliver();

    review.hold(beui::Key::Space, true);
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "beui-demo.a_new_page: added",
        "a painting that is new has nothing before it"
    );
    assert_eq!(
        review.requested(),
        ["before-checklist.an_old_list", "before-beui-demo.buttons"],
        "holding space fetches what the neighbours were, the hovered painting's sides having \
         been fetched already"
    );
    review
        .test
        .snapshot("holding_space_shows_what_the_paintings_were");

    review.hold(beui::Key::Space, false);
    assert_eq!(
        review.text("paint_review.preview.caption"),
        "beui-demo.a_new_page: after, 240 × 160",
        "letting go shows what it became again"
    );
}
