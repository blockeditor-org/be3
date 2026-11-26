use super::*;
use beui_host::native::testing::Target;

#[test]
fn a_tab_dropped_in_the_middle_of_the_desk_fills_it_and_one_at_its_edge_splits_it() {
    let mut test = alone(WIDE, Page::WindowManager);
    test.settle();
    assert!(test.shown("wm.desk"), "the empty desk shows its empty view");
    assert!(
        !test.shown("dock.grip.desk"),
        "the empty desk draws no bar, so no grip"
    );

    let desk = test.rect_of("wm.desk");
    let middle = pos2(desk.left() + desk.width() * 0.75, desk.center().y);
    test.drag(Target::text("Tab \"Sketch\""), Target::point(middle));
    test.frame(Vec::new());
    test.settle();
    let sketch = test.rect_of("wm.window.3");
    assert!(
        sketch.width() > desk.width() * 0.9,
        "the tab dropped in the middle fills the desk: {sketch:?} in {desk:?}"
    );
    assert!(
        test.shown("dock.grip.desk") && !test.shown("wm.desk"),
        "holding a tab, the desk shows its bar and no longer its empty view"
    );

    let edge = pos2(desk.right() - 8.0, desk.center().y);
    test.drag(Target::text("Tab \"Notes\""), Target::point(edge));
    test.frame(Vec::new());
    test.settle();
    let sketch = test.rect_of("wm.window.3");
    let notes = test.rect_of("wm.window.1");
    assert!(
        notes.left() >= sketch.right() - 0.5 && sketch.width() < desk.width() * 0.6,
        "the tab dropped at the desk's right edge splits it beside the first: {sketch:?} {notes:?}"
    );
    test.snapshot("a_tab_dropped_in_the_middle_of_the_desk_fills_it_and_one_at_its_edge_splits_it");
}
