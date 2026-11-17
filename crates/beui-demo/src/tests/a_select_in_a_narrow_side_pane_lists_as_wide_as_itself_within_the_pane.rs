use super::*;

#[test]
fn a_select_in_a_narrow_side_pane_lists_as_wide_as_itself_within_the_pane() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let pane = test.rect_of("bars.note");
    let trigger = test.rect_of("bars.blend");
    assert!(
        trigger.width() > 220.0,
        "the select fills the pane, wider than a select's least list: {trigger:?}"
    );

    test.click("bars.blend");
    test.frame(Vec::new());
    test.settle();
    let list = test.rect_of("select.list");
    assert_eq!(
        list.width(),
        trigger.width(),
        "its list is as wide as the select"
    );
    assert_eq!(list.left(), trigger.left(), "and lines up with it");
    assert!(
        list.top() >= trigger.bottom() && list.bottom() <= pane.bottom(),
        "it opens below the select and keeps within the pane, clear of the status band: {list:?} in {pane:?}"
    );
    test.snapshot("a_select_in_a_narrow_side_pane_lists_as_wide_as_itself_within_the_pane");
}
