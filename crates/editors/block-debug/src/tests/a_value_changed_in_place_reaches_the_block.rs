use super::*;

#[test]
fn a_value_changed_in_place_reaches_the_block() {
    let mut test = editor();
    test.click("block-debug.unlock");
    test.run();
    assert!(
        !test.shown("block-debug.unlock"),
        "the warning is dismissed"
    );
    test.double_click(format!("{COUNT}.row").as_str());
    test.run();
    assert!(test.shown(&rename_field()), "the value opens for editing");
    test.snapshot("a_value_being_changed_in_place");
    test.key(Key::A, Modifiers::CTRL);
    test.type_text("12");
    test.key(Key::Enter, Modifiers::NONE);
    test.run();
    assert_eq!(
        test.content::<CounterContent>(None).root().value(),
        12,
        "the block holds the new value"
    );
    assert!(says(&test, "count 12"), "the view shows the new value");
}
