use super::*;
use beui::PointerButton;

#[test]
fn a_wide_tab_dragged_along_its_strip_passes_a_narrow_one() {
    let mut test = alone(WIDE, Page::Reordering);
    reveal(&mut test, "demo.reorder.tab.4");
    let grab = test.rect_of("demo.reorder.tab.1").center();
    let past = test.rect_of("demo.reorder.tab.2").right() + 10.0;
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(6.0, 0.0))]);
    test.frame(vec![Input::PointerMoved(pos2(past, grab.y + 6.0))]);
    test.settle();
    test.snapshot("a_wide_tab_dragged_along_its_strip_passes_a_narrow_one");
    test.up(pos2(past, grab.y), PointerButton::Primary);
}
