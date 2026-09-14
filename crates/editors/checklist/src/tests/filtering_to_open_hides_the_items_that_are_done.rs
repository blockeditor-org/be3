use super::*;

#[test]
fn filtering_to_open_hides_the_items_that_are_done() {
    let (mut editor, block) = editor(&[("buy milk", false), ("call the vet", false)]);

    editor.click("checklist.item.1.done");
    editor.run();
    editor.click("checklist.filter.open");
    editor.run();

    assert_eq!(
        items(&block),
        [
            ("buy milk".to_owned(), false),
            ("call the vet".to_owned(), true)
        ]
    );
    let ui = editor.app().ui().expect("the checklist ui is not open");
    assert_eq!(
        ui.document().find_test_id("checklist.item.1.done"),
        None,
        "the filtered out item must not leave its test id behind"
    );
    editor.snapshot("filtering_to_open_hides_the_items_that_are_done");
}
