use super::*;
use beui::PointerButton;
use beui::native::testing::Target;

fn choose(test: &mut DocumentTest, grip: &str, item: &str) {
    test.click_with(grip, PointerButton::Secondary);
    test.frame(Vec::new());
    test.click(Target::text(item));
    test.frame(Vec::new());
}

#[test]
fn a_pane_shows_its_tabs_as_icons_across_the_top_or_in_a_narrow_sidebar() {
    let mut test = demo(WIDE);
    open_beside(&mut test, Page::Buttons);
    choose(&mut test, "dock.grip.catalog", "Icons only");
    choose(&mut test, "dock.grip.pages", "Show tabs in a sidebar");
    let edge = test.rect_of("dock.sidebar.pages");

    test.down("dock.sidebar.pages", PointerButton::Primary);
    test.hover(Target::point(edge.center() - Vec2::new(160.0, 0.0)));
    test.settle();
    let narrow = test.rect_of("dock.sidebar.pages");
    assert!(
        narrow.left() < edge.left() - 100.0 && narrow.left() > edge.center().x - 160.0,
        "dragged past its narrowest, the sidebar settles as wide as its icons"
    );
    test.up(
        Target::point(edge.center() - Vec2::new(160.0, 0.0)),
        PointerButton::Primary,
    );
    test.frame(Vec::new());
    test.snapshot("docking_icons_only");
}
