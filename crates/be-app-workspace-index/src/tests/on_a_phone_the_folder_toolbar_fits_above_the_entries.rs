use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

#[test]
fn on_a_phone_the_folder_toolbar_fits_above_the_entries() {
    let (Fixture { mut editor, .. }, children) = editor(8);
    editor.set_size(PHONE);
    editor.settle();
    let last = editor.rect_of("folder.direction");
    assert!(
        last.right() <= PHONE.x + 0.5,
        "every control of the toolbar fits across a phone, {last:?}"
    );
    assert!(editor.shown(&format!("folder.entry.{}", children[0])));
    editor.snapshot("on_a_phone_the_folder_toolbar_fits_above_the_entries");

    editor.set_size(LANDSCAPE_PHONE);
    editor.settle();
    assert!(editor.shown("folder.toolbar"));
    assert!(editor.shown(&format!("folder.entry.{}", children[0])));
    editor.snapshot("on_a_phone_on_its_side_the_folder_toolbar_fits_above_the_entries");
}
