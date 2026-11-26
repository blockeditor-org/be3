use super::*;

#[test]
fn a_dialog_hangs_from_the_tab_that_opened_it() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.open");

    test.click("demo.dialog.open");
    test.frame(Vec::new());
    let dialog = dialog_id(0);
    assert!(test.shown(&dialog), "the dialog opens over the page");
    test.snapshot("docking_dialog");

    open_beside(&mut test, Page::Text);
    assert!(
        !test.shown(&dialog),
        "showing another page in its place hides the dialog"
    );

    open(&mut test, Page::Docking);
    assert!(test.shown(&dialog), "the dialog comes back with its page");
}
