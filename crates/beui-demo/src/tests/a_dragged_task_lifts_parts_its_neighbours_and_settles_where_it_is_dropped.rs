use super::*;
use beui::PointerButton;
use std::time::Duration;

#[test]
fn a_dragged_task_lifts_parts_its_neighbours_and_settles_where_it_is_dropped() {
    let mut test = alone(WIDE, Page::Reordering);
    reveal(&mut test, "demo.reorder.task.4");
    let grab = test.rect_of("demo.reorder.task.0").center();
    let below = test.rect_of("demo.reorder.task.2").center();
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(0.0, 6.0))]);
    test.frame(vec![Input::PointerMoved(below + Vec2::new(0.0, 4.0))]);
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.up(below + Vec2::new(0.0, 4.0), PointerButton::Primary);
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.settle();
    test.snapshot("a_dragged_task_lifts_parts_its_neighbours_and_settles_where_it_is_dropped");
}
