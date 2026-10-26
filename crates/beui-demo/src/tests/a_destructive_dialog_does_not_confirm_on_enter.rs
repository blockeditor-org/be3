use super::*;
use beui::Key;

#[test]
fn a_destructive_dialog_does_not_confirm_on_enter() {
    let mut test = alone(WIDE, Page::Overlays);
    let root = test.document().root().expect("the demo built a root");
    reveal(&mut test, "demo.discard.open");
    test.click("demo.discard.open");
    test.frame(Vec::new());
    test.snapshot("a_destructive_dialog_does_not_confirm_on_enter");

    press_key(&mut test, Key::Enter);
    test.settle();
    assert!(
        !test.shows("demo.discard.confirm"),
        "Enter presses Cancel, where the focus starts, and closes the dialog"
    );
    assert_eq!(
        showing(test.document(), root, "Nothing discarded"),
        1,
        "and discards nothing"
    );

    test.click("demo.discard.open");
    test.frame(Vec::new());
    test.click("demo.discard.confirm");
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.discard.confirm"),
        "a click on Discard closes it"
    );
    assert_eq!(showing(test.document(), root, "Discarded the changes"), 1);
}
