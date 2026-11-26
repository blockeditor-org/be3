use super::*;
use beui_host::native::testing::Target;

#[test]
fn a_tab_carried_into_another_panes_bar_parts_it_and_the_dock_closes_its_pane() {
    let mut test = alone(WIDE, Page::WindowManager);
    test.settle();
    let desk = test.rect_of("wm.desk");
    let middle = pos2(desk.left() + desk.width() * 0.75, desk.center().y);
    test.drag(Target::text("Tab \"Sketch\""), Target::point(middle));
    test.frame(Vec::new());
    test.settle();
    let edge = pos2(desk.right() - 8.0, desk.center().y);
    test.drag(Target::text("Tab \"Notes\""), Target::point(edge));
    test.frame(Vec::new());
    test.settle();
    let split = test.rect_of("wm.window.3");

    // Carried into the other pane's bar, just past its tab, the bar opens a gap for it and
    // the dock moves aside as the move would leave it: the pane it leaves closes into the
    // one it joins.
    let sketch = test.point(Target::text("Tab \"Sketch\""));
    let from = test.point(Target::text("Tab \"Notes\""));
    let into = sketch + Vec2::new(110.0, 0.0);
    test.down(Target::point(from), beui::PointerButton::Primary);
    for step in 1..=8 {
        test.hover(Target::point(from + (into - from) * (step as f32 / 8.0)));
    }
    test.pause(std::time::Duration::from_millis(400));
    let gap = test.point(Target::text("Tab \"Sketch\""));
    assert!(
        (gap.y - sketch.y).abs() < 1.0,
        "the bar the tab is carried into stays where it was"
    );
    assert_eq!(
        test.rect_of("wm.window.3").size(),
        split.size(),
        "the panel of the pane it joins keeps the size it was laid out at while the dock moves"
    );
    test.snapshot("a_tab_carried_into_another_panes_bar_parts_it_and_the_dock_closes_its_pane");

    test.up(Target::point(into), beui::PointerButton::Primary);
    test.frame(Vec::new());
    test.settle();
    let joined = test.rect_of("wm.window.1");
    assert!(
        joined.width() > desk.width() * 0.9,
        "let go, the tab joins the pane, which fills the desk: {joined:?} in {desk:?}"
    );
    assert!(
        !test.shown("wm.window.3"),
        "and shows the tab carried into it, its first tab behind"
    );
}
