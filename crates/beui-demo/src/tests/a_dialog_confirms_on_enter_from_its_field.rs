use super::*;
use beui::Key;

#[test]
fn a_dialog_confirms_on_enter_from_its_field() {
    let mut test = alone(WIDE, Page::Overlays);
    let root = test.document().root().expect("the demo built a root");
    reveal(&mut test, "demo.rename.open");
    test.click("demo.rename.open");
    test.frame(Vec::new());
    test.click("demo.rename.field");
    press_key(&mut test, Key::End);
    test.frame(vec![Input::Text(" 2".to_owned())]);
    test.frame(Vec::new());
    test.snapshot("a_dialog_confirms_on_enter_from_its_field");

    press_key(&mut test, Key::Enter);
    assert!(
        !test.shows("demo.rename.confirm"),
        "Enter closes the dialog"
    );
    assert_eq!(
        showing(test.document(), root, "Notes 2"),
        1,
        "Enter runs the dialog's confirm action"
    );
}
