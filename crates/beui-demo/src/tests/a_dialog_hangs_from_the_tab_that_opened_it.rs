use super::*;

#[test]
fn a_dialog_hangs_from_the_tab_that_opened_it() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.open");

    test.click("demo.dialog.open");
    test.frame(Vec::new());
    let dialog = dialog_id(0);
    assert!(test.shows(&dialog), "the dialog opens over the page");
    test.snapshot("docking_dialog");

    open(&mut test, Page::Text);
    assert!(
        !test.shows(&dialog),
        "showing another page in its place hides the dialog"
    );

    open(&mut test, Page::Docking);
    assert!(test.shows(&dialog), "the dialog comes back with its page");
}
