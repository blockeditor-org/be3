use super::*;
use beui::PointerButton;
use beui_host::native::testing::Target;

#[test]
fn the_nested_dock_pops_out_into_a_window_from_its_tabs_menu() {
    let mut test = demo(WIDE);
    open(&mut test, Page::NestedDocks);
    test.frame(Vec::new());
    let close = format!("dock.tab.{}.close", tab(Page::NestedDocks));
    let grip = format!("dock.grip.joined:{}/outline", tab(Page::NestedDocks));
    let docked = test.rect_of(&grip);

    let group = Target::point(test.rect_of(&close).center() - Vec2::new(60.0, 0.0));
    test.click_with(group, PointerButton::Secondary);
    test.frame(Vec::new());
    test.click(Target::text("Pop out into a window"));
    test.frame(Vec::new());
    test.settle();

    assert_ne!(
        test.rect_of(&grip),
        docked,
        "the group's panes moved out of the demo's dock into a window"
    );
    let root = test.document().root().expect("the demo built a root");
    for title in ["Outline", "Preview", "Notes"] {
        assert!(
            showing(test.document(), root, title) >= 1,
            "the window shows {title}"
        );
    }
    test.snapshot("the_nested_dock_pops_out_into_a_window_from_its_tabs_menu");
}
