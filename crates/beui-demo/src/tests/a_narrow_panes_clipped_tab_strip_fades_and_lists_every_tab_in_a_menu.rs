use super::*;
use beui_host::native::testing::Target;

const NARROW_PANE: f32 = 340.0;
const ALL_TABS: &str = "dock.pages.all-tabs";

#[test]
fn a_narrow_panes_clipped_tab_strip_fades_and_lists_every_tab_in_a_menu() {
    let mut test = demo(WIDE);
    open_beside(&mut test, Page::Text);
    open_beside(&mut test, Page::Buttons);
    open_beside(&mut test, Page::Inputs);
    assert!(
        !test.shown(ALL_TABS),
        "a strip with room for its tabs offers no menu of them"
    );

    let splitter = test.rect_of("dock.splitter.shell").center();
    test.drag(
        Target::point(splitter),
        Target::point(pos2(WIDE.x - NARROW_PANE, splitter.y)),
    );
    test.settle();
    assert!(
        test.shown(ALL_TABS),
        "a strip too short for its tabs ends in a menu of them"
    );
    let first = format!("dock.tab.{}.close", tab(Page::Docking));
    assert!(test.shown(&first), "the strip still starts at its first tab");
    test.record();

    test.click(ALL_TABS);
    test.settle();
    let row = format!("{ALL_TABS}.tab.{}", tab(Page::Buttons));
    assert!(test.shown(&row), "the menu lists every tab");
    test.record();
    test.click(row.as_str());
    test.settle();
    test.pause(std::time::Duration::from_millis(500));
    test.record();
    test.snapshot("a_narrow_panes_clipped_tab_strip_fades_and_lists_every_tab_in_a_menu");
    let buttons = format!("dock.tab.{}.close", tab(Page::Buttons));
    let shown = test.rect_of(&buttons);
    let all = test.rect_of(ALL_TABS);
    assert!(
        shown.right() <= all.left(),
        "choosing a tab from the menu shows it and scrolls it into view: {shown:?} beside {all:?}"
    );
}
