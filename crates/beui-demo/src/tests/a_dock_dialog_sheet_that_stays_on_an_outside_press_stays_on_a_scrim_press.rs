use super::*;

#[test]
fn a_dock_dialog_sheet_that_stays_on_an_outside_press_stays_on_a_scrim_press() {
    let mut test = demo(PHONE);
    open(&mut test, Page::Docking);
    reveal(&mut test, "demo.dialog.open");
    test.click("demo.dialog.open");
    settle(&mut test);
    let sheet = format!("{}.sheet", dialog_id(0));
    assert!(test.shown(&sheet));

    test.click(pos2(PHONE.x / 2.0, 40.0));
    settle(&mut test);
    assert!(
        test.shown(&sheet),
        "a dialog that does not close on an outside press stays, as its window would"
    );
}
