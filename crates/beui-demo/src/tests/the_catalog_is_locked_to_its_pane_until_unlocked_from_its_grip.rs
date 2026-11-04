use super::*;
use beui::PointerButton;
use beui_host::native::testing::Target;

#[test]
fn the_catalog_is_locked_to_its_pane_until_unlocked_from_its_grip() {
    let mut test = demo(WIDE);
    let pages = test.rect_of("dock.grip.pages");
    let into_pages = Target::point(pages.center() + Vec2::new(200.0, 200.0));
    test.drag(Target::text("Tab \"Components\""), into_pages.clone());
    test.settle();
    assert!(
        text_x(&test, "Components") < pages.left(),
        "the catalog's tab stays in its locked area"
    );

    test.click_with("dock.grip.catalog", PointerButton::Secondary);
    test.frame(Vec::new());
    assert!(
        menu_shows(&test, "Locked"),
        "the catalog's grip offers its lock"
    );
    test.snapshot("docking_locked_catalog_menu");
    test.click(Target::text("MenuItemCheckBox \"Locked\""));
    test.frame(Vec::new());

    test.drag(Target::text("Tab \"Components\""), into_pages);
    test.settle();
    assert!(
        !test.shows("dock.grip.catalog"),
        "unlocked, its tab moves into the pages and its emptied pane closes"
    );
}
