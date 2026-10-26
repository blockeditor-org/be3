use block_editor_beui::BlockLocation;
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn on_a_desktop_a_touch_opens_the_add_menu_as_a_sheet_and_a_mouse_as_a_dropdown() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());

    fixture
        .test
        .request_pick(5, filter(), BlockLocation::Root, None);
    fixture.test.settle();
    assert!(fixture.test.shown("picker.more"));
    assert!(
        !fixture.test.shown("sheet.handle"),
        "a pick a mouse asked for opens the menu as a dropdown"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.test.settle();
    assert!(!fixture.test.shown("picker.more"));

    fixture
        .test
        .request_touch_pick(6, filter(), BlockLocation::Root, None);
    fixture.test.settle();
    assert!(
        fixture.test.shown("sheet.handle") && fixture.test.shown("picker.more"),
        "a pick a finger asked for opens the menu as a sheet, on a desktop too"
    );
    fixture
        .test
        .snapshot("on_a_desktop_a_touch_opens_the_add_menu_as_a_sheet");
}
