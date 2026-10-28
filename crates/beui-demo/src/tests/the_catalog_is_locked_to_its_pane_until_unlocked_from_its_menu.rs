use super::*;
use beui::PointerButton;
use beui::native::testing::Target;

fn open_tab_menu(test: &mut DocumentTest) {
    let grip = test.rect_of("dock.grip.catalog");
    let tab = Target::point(grip.center() + Vec2::new(70.0, 0.0));
    test.click_with(tab, PointerButton::Secondary);
    test.frame(Vec::new());
}

fn menu_shows(test: &DocumentTest, text: &str) -> bool {
    let document = test.document();
    document
        .layers()
        .iter()
        .filter_map(|layer| layer.content)
        .any(|content| showing(document, content, text) > 0)
}

#[test]
fn the_catalog_is_locked_to_its_pane_until_unlocked_from_its_menu() {
    let mut test = demo(WIDE);
    open_tab_menu(&mut test);
    assert!(
        menu_shows(&test, "Unlock"),
        "the catalog's tab is locked to its pane"
    );
    test.snapshot("docking_locked_catalog_menu");

    test.drive(&["--no-settle", "click", "Unlock"]);
    test.frame(Vec::new());
    open_tab_menu(&mut test);
    assert!(
        menu_shows(&test, "Lock"),
        "unlocked, its menu offers to lock it again"
    );
}
