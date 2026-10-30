use super::*;

#[test]
fn a_dialog_from_a_shortcut_points_at_its_tab_and_in_fullscreen_opens_at_the_top() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.tab-settings");
    test.click("demo.tab-settings");
    test.frame(Vec::new());
    test.settle();
    let tab = test.rect_of(&format!("dock.tab.{}", tab(Page::Docking)));
    let body = test.rect_of("demo.tab-settings.body");
    assert!(
        body.top() > tab.bottom() && body.top() < tab.bottom() + 80.0,
        "the dialog opens just under its tab: {body:?} under {tab:?}"
    );
    assert!(
        body.left() < tab.center().x && body.right() > tab.center().x,
        "around the middle of it: {body:?} for {tab:?}"
    );
    test.snapshot("a_dialog_from_a_shortcut_points_at_its_tab");
    press_key(&mut test, beui::Key::Escape);
    assert!(!test.shows("demo.tab-settings.body"), "Escape closes it");

    let page = pos2(WIDE.x * 0.75, WIDE.y / 2.0);
    for _ in 0..REVEAL_TICKS {
        if test.rect_of("demo.fullscreen").top() > tab.bottom() + 60.0 {
            break;
        }
        test.scroll(page, Vec2::new(0.0, -REVEAL_STEP));
    }
    test.click("demo.fullscreen");
    test.frame(Vec::new());
    reveal(&mut test, "demo.tab-settings");
    test.click("demo.tab-settings");
    test.frame(Vec::new());
    test.settle();
    let body = test.rect_of("demo.tab-settings.body");
    assert!(
        (body.center().x - WIDE.x / 2.0).abs() < 2.0 && body.top() < 120.0,
        "with no tab showing, it opens at the top centre of the page: {body:?}"
    );
    test.snapshot("a_dialog_from_a_shortcut_in_a_fullscreen_tab_opens_at_the_top");
}
