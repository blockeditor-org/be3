use super::*;

#[test]
fn j_and_k_move_through_the_paintings_in_order() {
    let mut review = Review::open(Some("beui-demo.a_new_page"));
    review.load(mixed());
    let mut seen = vec![review.shown_name()];
    for _ in 0..5 {
        review.press(beui::Key::J);
        seen.push(review.shown_name());
    }
    assert_eq!(
        seen,
        [
            "beui-demo.a_new_page",
            "checklist.an_old_list",
            "beui-demo.buttons",
            "text-block.typing_a_paragraph",
            "map.broken",
            "map.broken",
        ],
        "J walks the paintings in the grid's order and stops at the end"
    );

    review.press(beui::Key::K);
    assert_eq!(review.shown_name(), "text-block.typing_a_paragraph");
}
