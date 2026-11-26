use super::*;
use beui_host::native::testing::Target;

#[test]
fn a_tab_carried_to_a_panes_edge_parts_the_dock_while_its_panels_keep_their_size() {
    let mut test = alone(WIDE, Page::WindowManager);
    test.settle();
    let desk = test.rect_of("wm.desk");
    let middle = pos2(desk.left() + desk.width() * 0.75, desk.center().y);
    test.drag(Target::text("Tab \"Sketch\""), Target::point(middle));
    test.frame(Vec::new());
    test.settle();
    let filled = test.rect_of("wm.window.3");

    // Carried to the desk's right edge, the desk parts as splitting it there would leave it.
    let from = test.point(Target::text("Tab \"Notes\""));
    let edge = pos2(desk.right() - 8.0, desk.center().y);
    test.down(Target::point(from), beui::PointerButton::Primary);
    for step in 1..=8 {
        test.hover(Target::point(from + (edge - from) * (step as f32 / 8.0)));
    }
    test.pause(std::time::Duration::from_millis(400));
    assert!(
        test.shown("dock.drop"),
        "the carried tab is drawn the size of the opening it would make"
    );
    let opening = test.rect_of("dock.drop");
    assert!(
        opening.width() < desk.width() * 0.6 && opening.width() > desk.width() * 0.3,
        "the opening is the half of the desk the split would give it: {opening:?} in {desk:?}"
    );
    let held = test.rect_of("wm.window.3");
    assert_eq!(
        held.size(),
        filled.size(),
        "the panel beside it keeps the size it was laid out at while its pane moves"
    );
    test.snapshot("a_tab_carried_to_a_panes_edge_parts_the_dock");

    test.up(Target::point(edge), beui::PointerButton::Primary);
    test.frame(Vec::new());
    test.settle();
    let sketch = test.rect_of("wm.window.3");
    let notes = test.rect_of("wm.window.1");
    assert!(
        notes.left() >= sketch.right() - 0.5 && sketch.width() < desk.width() * 0.6,
        "let go, the split is made and the panel takes its new size: {sketch:?} {notes:?}"
    );
    assert!(!test.shown("dock.drop"), "and the opening has landed");
}
