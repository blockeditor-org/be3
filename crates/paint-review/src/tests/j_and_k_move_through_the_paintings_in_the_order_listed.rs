use super::*;

#[test]
fn j_and_k_move_through_the_paintings_in_the_order_listed() {
    let mut review = Review::with(mixed());
    let mut seen = vec![review.shown_name()];
    for _ in 0..5 {
        review.press(beui::Key::J);
        seen.push(review.shown_name());
    }
    assert_eq!(
        seen,
        [
            "beui-demo.buttons",
            "text-block.typing_a_paragraph",
            "beui-demo.a_new_page",
            "checklist.an_old_list",
            "map.broken",
            "map.broken",
        ],
        "J walks the changed, then the added, removed and unreadable paintings, and stops at the end"
    );

    review.press(beui::Key::K);
    assert_eq!(review.shown_name(), "checklist.an_old_list");
    review.deliver();
    review
        .test
        .snapshot("j_and_k_move_through_the_paintings_in_the_order_listed");
}
