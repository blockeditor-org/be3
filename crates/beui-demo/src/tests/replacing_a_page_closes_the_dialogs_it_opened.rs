use super::*;

#[test]
fn replacing_a_page_closes_the_dialogs_it_opened() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.open");
    test.click("demo.dialog.open");
    test.frame(Vec::new());
    let dialog = dialog_id(0);
    assert!(test.shown(&dialog), "the dialog opens over the page");

    open(&mut test, Page::Text);
    open(&mut test, Page::Docking);
    assert!(
        !test.shown(&dialog),
        "a page shown in place of the one that opened the dialog closes it, so it is gone when that page opens again"
    );
}
