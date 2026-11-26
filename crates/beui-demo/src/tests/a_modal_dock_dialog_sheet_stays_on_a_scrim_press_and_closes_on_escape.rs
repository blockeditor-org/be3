use super::*;

#[test]
fn a_modal_dock_dialog_sheet_stays_on_a_scrim_press_and_closes_on_escape() {
    let mut test = demo(PHONE);
    open(&mut test, Page::Docking);
    reveal(&mut test, "demo.dialog.modal");
    test.click("demo.dialog.modal");
    settle(&mut test);
    let sheet = format!("{}.sheet", dialog_id(0));
    assert!(test.shown(&sheet), "the modal dialog is a sheet on a phone");

    test.click(pos2(PHONE.x / 2.0, 40.0));
    settle(&mut test);
    assert!(
        test.shown(&sheet),
        "a press on the scrim calls attention to a modal dialog rather than closing it"
    );

    test.key(beui::Key::Escape, beui::Modifiers::NONE);
    settle(&mut test);
    assert!(!test.shown(&sheet), "Escape closes it");
}
