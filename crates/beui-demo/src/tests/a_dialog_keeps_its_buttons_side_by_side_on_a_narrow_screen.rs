use super::*;

#[test]
fn a_dialog_keeps_its_buttons_side_by_side_on_a_narrow_screen() {
    let mut test = alone(Vec2::new(320.0, 640.0), Page::Overlays);
    reveal(&mut test, "demo.rename.open");
    test.click("demo.rename.open");
    test.frame(Vec::new());
    let cancel = test.rect_of("demo.rename.cancel");
    let confirm = test.rect_of("demo.rename.confirm");
    assert_eq!(confirm.top(), cancel.top(), "Rename sits beside Cancel");
    assert!(
        confirm.left() >= cancel.right(),
        "Rename is right of Cancel, at {confirm:?} and {cancel:?}"
    );
    test.snapshot("a_dialog_keeps_its_buttons_side_by_side_on_a_narrow_screen");
}
