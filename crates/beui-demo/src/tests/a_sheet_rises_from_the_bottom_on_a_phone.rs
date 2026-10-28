use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

#[test]
fn a_sheet_rises_from_the_bottom_on_a_phone() {
    let mut test = demo(PHONE);
    open(&mut test, Page::Docking);
    reveal(&mut test, "demo.dialog.card");

    test.click("demo.dialog.card");
    test.settle();
    let sheet = dialog_id(0);
    assert!(test.shown(&sheet), "the sheet rises from the bottom");
    test.snapshot("docking_sheet_phone");

    press_key(&mut test, beui::Key::Escape);
    test.settle();
    assert!(!test.shown(&sheet), "Escape closes it");
}
