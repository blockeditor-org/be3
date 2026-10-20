use super::*;

#[test]
fn a_sheet_hangs_from_its_tab_and_waits_while_another_tab_is_shown() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.sheet");

    test.click("demo.dialog.sheet");
    test.frame(Vec::new());
    let sheet = dialog_id(0);
    let waiting = format!("dock.tab.{}.waiting", Page::Docking.tab().value());
    let count = format!("demo.sheet.{}.count", crate::DIALOG_TABS);
    assert!(test.shows(&sheet), "the sheet opens under the tab bar");
    assert!(test.shows(&waiting), "its tab waits on it");
    test.snapshot("docking_sheet");

    test.click("demo.dialog.open");
    test.frame(Vec::new());
    assert!(
        !test.shows(&dialog_id(1)),
        "the tab under the sheet takes no press"
    );
    test.click(&count);
    test.frame(Vec::new());
    let pressed = test.document().find_test_id(&count);

    open_beside(&mut test, Page::Text);
    assert!(
        test.shows(&format!("dock.tab.{}.close", Page::Text.tab().value())),
        "the catalog beside the sheet still opens a page"
    );
    assert!(
        !test.shows(&sheet),
        "the sheet hides while another tab is shown"
    );
    assert!(test.shows(&waiting), "and its tab still waits on it");
    test.snapshot("docking_sheet_waiting");

    open(&mut test, Page::Docking);
    assert!(test.shows(&sheet), "the sheet comes back with its tab");
    assert_eq!(
        test.document().find_test_id(&count),
        pressed,
        "holding what it held"
    );

    press_key(&mut test, beui::Key::Escape);
    assert!(!test.shows(&sheet), "Escape closes the sheet");
    assert!(!test.shows(&waiting), "and the tab stops waiting");
}
