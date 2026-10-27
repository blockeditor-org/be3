use super::*;

#[test]
fn a_card_floats_from_its_tab_and_waits_while_another_tab_is_shown() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.card");

    test.click("demo.dialog.card");
    test.frame(Vec::new());
    let card = dialog_id(0);
    let waiting = format!("dock.tab.{}.waiting", tab(Page::Docking));
    assert!(test.shown(&card), "the card opens under the tab bar");
    assert!(test.shown(&waiting), "its tab waits on it");
    test.snapshot("docking_card");

    test.click("demo.dialog.open");
    test.frame(Vec::new());
    assert!(
        !test.shown(&dialog_id(1)),
        "the tab under the card takes no press"
    );
    assert!(
        !test.shown(&card),
        "a press on the dimmed tab closes the card"
    );
    assert!(!test.shown(&waiting), "and the tab stops waiting");

    test.click("demo.dialog.card");
    test.frame(Vec::new());
    let card = dialog_id(1);
    let count = format!("demo.card.{}.count", crate::DIALOG_TABS + 1);
    assert!(test.shown(&card), "the card opens again");
    test.click(&count);
    test.frame(Vec::new());
    let pressed = test.document().find_test_id(&count);

    open_beside(&mut test, Page::Text);
    assert!(
        test.shown(&format!("dock.tab.{}.close", tab(Page::Text))),
        "the catalog beside the card still opens a page"
    );
    assert!(
        !test.shown(&card),
        "the card hides while another tab is shown"
    );
    assert!(test.shown(&waiting), "and its tab still waits on it");
    test.snapshot("docking_card_waiting");

    open(&mut test, Page::Docking);
    assert!(test.shown(&card), "the card comes back with its tab");
    assert_eq!(
        test.document().find_test_id(&count),
        pressed,
        "holding what it held"
    );

    press_key(&mut test, beui::Key::Escape);
    assert!(!test.shown(&card), "Escape closes the card");
    assert!(!test.shown(&waiting), "and the tab stops waiting");
}
