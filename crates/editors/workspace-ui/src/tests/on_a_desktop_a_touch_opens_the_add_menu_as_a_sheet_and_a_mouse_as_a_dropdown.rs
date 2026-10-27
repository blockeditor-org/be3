use block_editor_beui::BlockLocation;
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn on_a_desktop_a_touch_opens_the_add_menu_as_a_sheet_and_a_mouse_as_a_dropdown() {
    let (mut workspace, _) = editor();
    workspace.test.catalog(catalog());

    let mut fixture = picking(&workspace, filter(), BlockLocation::Root);
    fixture.test.settle();
    assert!(fixture.test.shown("picker.more"));
    assert!(
        !fixture.test.shown("sheet.handle"),
        "a pick a mouse asked for opens the menu as a dropdown"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.test.settle();
    assert!(!fixture.test.shown("picker.more"));
    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(None)],
        "escaping the menu cancels the pick"
    );

    let mut fixture = picking_with(&workspace, filter(), BlockLocation::Root, true);
    fixture.test.settle();
    assert!(
        fixture.test.shown("sheet.handle") && fixture.test.shown("picker.more"),
        "a pick a finger asked for opens the menu as a sheet, on a desktop too"
    );
    fixture
        .test
        .snapshot("on_a_desktop_a_touch_opens_the_add_menu_as_a_sheet");
}
