use super::*;

#[test]
fn a_modal_dialog_blocks_its_tab_and_stacks_a_nested_one() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.modal");

    test.click("demo.dialog.modal");
    test.frame(Vec::new());
    let (outer, inner) = (dialog_id(0), dialog_id(1));
    assert!(test.shows(&outer), "the modal dialog opens over the page");

    test.click("demo.dialog.open");
    test.frame(Vec::new());
    assert!(
        !test.shows(&inner),
        "the page under a modal dialog takes no press"
    );

    test.click(format!("demo.dialog.{}.nested", crate::DIALOG_TABS));
    test.frame(Vec::new());
    assert!(test.shows(&inner), "a dialog opens over the dialog");
    test.advance(std::time::Duration::from_secs(1));
    test.frame(Vec::new());
    test.snapshot("docking_modal_dialog");

    test.click(format!("{outer}.close"));
    test.frame(Vec::new());
    assert!(
        test.shows(&outer),
        "the dialog under a modal one takes no press either"
    );

    test.click(format!("demo.dialog.{}.close", crate::DIALOG_TABS + 1));
    test.click(format!("demo.dialog.{}.close", crate::DIALOG_TABS));
    test.frame(Vec::new());
    assert!(!test.shows(&outer), "both dialogs close");

    test.click("demo.dialog.open");
    test.frame(Vec::new());
    assert!(
        test.shows(&dialog_id(2)),
        "with the dialogs closed, the page takes presses again"
    );
}
