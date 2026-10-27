use super::*;
use beui::PointerButton;

#[test]
fn a_task_carried_into_the_other_list_parts_it_where_it_would_land() {
    let mut test = alone(WIDE, Page::Reordering);
    reveal(&mut test, "demo.reorder.card.4");
    let grab = test.rect_of("demo.reorder.card.0").center();
    let into = test.rect_of("demo.reorder.card.4").center() - Vec2::new(0.0, 4.0);
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(6.0, 0.0))]);
    test.frame(vec![Input::PointerMoved(into)]);
    test.settle();
    test.snapshot("a_task_carried_into_the_other_list_parts_it_where_it_would_land");
    test.up(into, PointerButton::Primary);
    test.settle();
    test.snapshot("a_task_carried_into_the_other_list_parts_it_where_it_would_land_dropped");
}
