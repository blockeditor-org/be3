use super::*;
use beui_host::native::testing::Target;

const NARROW_PANE: f32 = 340.0;

#[test]
fn a_narrow_pane_moves_its_labelled_pin_into_its_menu() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Docking);
    assert!(
        test.shown("dock.pages.pin.demo.reset"),
        "a wide pane's bar holds the action pinned with a label"
    );

    let splitter = test.rect_of("dock.splitter.shell").center();
    test.drag(
        Target::point(splitter),
        Target::point(pos2(WIDE.x - NARROW_PANE, splitter.y)),
    );
    test.settle();
    assert!(
        test.shown("dock.pages.pin.demo.edit"),
        "a narrow pane's bar keeps the action pinned as an icon"
    );
    assert!(
        !test.shown("dock.pages.pin.demo.reset"),
        "but leaves out the one pinned with a label"
    );
    test.snapshot("docking_narrow_pane");

    test.click("dock.pages.menu");
    test.settle();
    assert!(
        test.shown("dock.pages.menu.demo.reset"),
        "and its menu offers it instead"
    );
}
