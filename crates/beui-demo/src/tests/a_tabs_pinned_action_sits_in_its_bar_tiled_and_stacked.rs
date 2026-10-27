use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

#[test]
fn a_tabs_pinned_action_sits_in_its_bar_tiled_and_stacked() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Docking);
    assert!(
        test.shown("dock.pin.demo.edit"),
        "the docking page pins its Edit action in the bar of the tab it is shown in"
    );
    test.snapshot("a_pinned_action_in_a_tiled_bar");

    let mut test = demo(PHONE);
    open(&mut test, Page::Docking);
    assert!(
        test.shown("dock.pin.demo.edit"),
        "the stacked bar holds it too"
    );
    test.click("dock.switch");
    test.settle();
    test.snapshot("a_pinned_action_in_a_stacked_bar_and_its_switcher");
}
