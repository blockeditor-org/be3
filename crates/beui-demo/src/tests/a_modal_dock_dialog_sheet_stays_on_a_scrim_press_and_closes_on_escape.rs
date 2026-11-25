use std::time::Duration;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

fn settle(test: &mut DocumentTest) {
    for _ in 0..30 {
        test.advance(Duration::from_millis(16));
        test.frame(Vec::new());
    }
}

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
