use super::*;
use beui::PointerButton;
use std::time::Duration;

#[test]
fn a_page_tab_dragged_along_its_bar_parts_the_tabs_it_passes() {
    let mut test = demo(WIDE);
    open_beside(&mut test, Page::Buttons);
    open_beside(&mut test, Page::Text);
    test.settle();
    let docking = test.rect_of(&format!("dock.tab.{}.close", tab(Page::Docking)));
    let text = test.rect_of(&format!("dock.tab.{}.close", tab(Page::Text)));
    let grab = docking.center() - Vec2::new(50.0, 0.0);
    let past = pos2(text.right() - 20.0, grab.y);
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(8.0, 0.0))]);
    test.record();
    for step in 1..=4 {
        let at = grab + (past - grab) * (step as f32 / 4.0);
        test.advance(Duration::from_millis(50));
        test.frame(vec![Input::PointerMoved(at)]);
        test.record();
    }
    for _ in 0..2 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.up(past, PointerButton::Primary);
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.settle();
    let landed = test.rect_of(&format!("dock.tab.{}.close", tab(Page::Docking)));
    assert!(
        (landed.left() - text.left()).abs() < 1.0,
        "the dragged page tab lands last, in the place of the last tab it passed"
    );
    test.snapshot("a_page_tab_dragged_along_its_bar_parts_the_tabs_it_passes");
}
