use super::*;
use beui::PointerButton;

#[test]
fn a_task_dragged_out_of_its_list_stays_in_it_at_the_end() {
    let mut test = alone(WIDE, Page::Reordering);
    reveal(&mut test, "demo.reorder.task.4");
    let grab = test.rect_of("demo.reorder.task.1").center();
    let away = test.rect_of("demo.reorder.task.4").center() + Vec2::new(380.0, 160.0);
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(0.0, 6.0))]);
    test.frame(vec![Input::PointerMoved(away)]);
    test.settle();
    test.snapshot("a_task_dragged_out_of_its_list_stays_in_it_at_the_end");
    test.up(away, PointerButton::Primary);
    test.settle();
    test.snapshot("a_task_dragged_out_of_its_list_stays_in_it_at_the_end_dropped");
}
