use super::*;
use beui::PointerButton;
use std::time::Duration;

#[test]
fn a_card_pulled_out_of_its_list_floats_under_the_pointer_as_the_gap_closes() {
    let mut test = alone(WIDE, Page::Reordering);
    reveal(&mut test, "demo.reorder.card.4");
    let grab = test.rect_of("demo.reorder.card.1").center();
    let card = test.rect_of("demo.reorder.card.1");
    let away = Pos2::new(card.right() + 8.0, grab.y + 30.0);
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(6.0, 0.0))]);
    test.frame(vec![Input::PointerMoved(away)]);
    test.record();
    for _ in 0..4 {
        test.advance(Duration::from_millis(40));
        test.frame(Vec::new());
        test.record();
    }
    test.settle();
    test.snapshot("a_card_pulled_out_of_its_list_floats_under_the_pointer_as_the_gap_closes");
    test.up(away, PointerButton::Primary);
    test.settle();
}
