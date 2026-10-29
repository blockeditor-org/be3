use super::*;
use block_editor_beui::beui::{PointerButton, Vec2};

#[test]
fn the_filmstrip_pops_out_into_a_window_once_its_area_is_unlocked() {
    let (mut test, editor) = editor(3);
    test.run();
    let docked = test.rect_of("presentation.filmstrip");
    let stage = editor.content_rect();
    assert!(docked.right() <= stage.left(), "the filmstrip stands left of the stage");

    test.click_with("dock.grip.filmstrip", PointerButton::Secondary);
    test.run();
    test.click("dock.pane.locked");
    test.run();
    let grip = test.rect_of("dock.grip.filmstrip");
    let tab = grip.center() + Vec2::new(grip.width() / 2.0 + 30.0, 0.0);
    test.click_with(tab, PointerButton::Secondary);
    test.run();
    test.click("dock.tab.pop-out");
    test.settle();

    assert!(
        editor.content_rect().width() > stage.width(),
        "the stage takes the deck's whole width"
    );
    assert!(test.shown("presentation.filmstrip"), "the filmstrip shows in its window");
    test.snapshot("the_filmstrip_pops_out_into_a_window_once_its_area_is_unlocked");
}
