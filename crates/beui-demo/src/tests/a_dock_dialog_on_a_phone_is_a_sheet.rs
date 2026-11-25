use std::time::Duration;

use super::*;

#[test]
fn a_dock_dialog_on_a_phone_is_a_sheet() {
    let size = Vec2::new(390.0, 844.0);
    let mut test = demo(size);
    open(&mut test, Page::Docking);
    reveal(&mut test, "demo.dialog.open");
    test.click("demo.dialog.open");
    for _ in 0..30 {
        test.advance(Duration::from_millis(16));
        test.frame(Vec::new());
    }
    let dialog = dialog_id(0);
    let sheet = test.rect_of(&format!("{dialog}.sheet"));
    assert!(
        sheet.width() > size.x * 0.8,
        "the dialog is a sheet across the phone: {sheet:?}"
    );
    assert!(
        sheet.bottom() > size.y * 0.75,
        "rising from the bottom: {sheet:?}"
    );
    assert!(
        !test.shown(&format!("{dialog}.header")),
        "with no window bar"
    );
    test.snapshot("docking_dialog_sheet");
}
