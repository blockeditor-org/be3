use super::*;

#[test]
fn adding_an_item_puts_it_on_the_list() {
    example_adding_an_item_puts_it_on_the_list();
}

fn example_adding_an_item_puts_it_on_the_list() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host, block);
    let mut test = BeuiTest::<ChecklistApp>::new(editor);
    test.store().register(checklist_types::registrations());
    test.hold(None, ChecklistContent::default());
    test.run();

    test.click("checklist.draft");
    test.run();
    test.type_text("buy milk");
    test.run();
    test.click("checklist.add");
    test.run();

    let items = test.content::<ChecklistContent>(None);
    assert_eq!(items.root().items[0].text, "buy milk");
    test.snapshot("adding_an_item_puts_it_on_the_list");
}
