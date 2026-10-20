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
    assert!(
        test.shows("demo.discard.confirm"),
        "Enter leaves the dialog open"
    );
    assert_eq!(showing(test.document(), root, "Nothing discarded"), 1);

    test.click("demo.discard.confirm");
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.discard.confirm"),
        "a click on Discard closes it"
    );
    assert_eq!(showing(test.document(), root, "Discarded the changes"), 1);
}
