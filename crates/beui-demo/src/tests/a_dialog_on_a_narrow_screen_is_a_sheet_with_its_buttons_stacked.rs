use super::*;

#[test]
fn a_dialog_on_a_narrow_screen_is_a_sheet_with_its_buttons_stacked() {
    let size = Vec2::new(320.0, 640.0);
    let mut test = alone(size, Page::Overlays);
    reveal(&mut test, "demo.rename.open");
    test.click("demo.rename.open");
    for _ in 0..30 {
        test.advance(Duration::from_millis(16));
        test.frame(Vec::new());
    }
    let cancel = test.rect_of("demo.rename.cancel");
    let confirm = test.rect_of("demo.rename.confirm");
    assert!(
        confirm.bottom() <= cancel.top(),
        "Rename comes first, above Cancel, at {confirm:?} and {cancel:?}"
    );
    assert_eq!(confirm.width(), cancel.width(), "both fill the width");
    assert!(
        confirm.width() > size.x / 2.0,
        "the buttons are as wide as the sheet: {confirm:?}"
    );
    assert!(
        cancel.bottom() > size.y * 0.75,
        "the sheet rises from the bottom: {cancel:?}"
    );
    test.snapshot("a_dialog_on_a_narrow_screen_is_a_sheet_with_its_buttons_stacked");
}
