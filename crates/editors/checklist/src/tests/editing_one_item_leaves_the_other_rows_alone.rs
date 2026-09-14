use super::*;

#[test]
fn editing_one_item_leaves_the_other_rows_alone() {
    let (mut editor, block) = editor(&[("buy milk", false), ("call the vet", false)]);
    let (first, second) = (id(&block, 0), id(&block, 1));
    editor.run();

    let rows = |editor: &mut BeuiTest<ChecklistApp>| {
        let ui = editor.app().ui().expect("the checklist ui is not open");
        [first, second].map(|item| {
            ui.document()
                .find_test_id(&format!("checklist.item.{item}.done"))
                .expect("a checklist row was never drawn")
        })
    };
    let before = rows(&mut editor);

    block.operate(ChecklistOperation::SetText {
        id: second,
        text: "call the vet back".to_owned(),
    });
    editor.run();

    assert_eq!(
        items(&block),
        [
            ("buy milk".to_owned(), false),
            ("call the vet back".to_owned(), false)
        ]
    );
    assert_eq!(
        rows(&mut editor),
        before,
        "editing one item must not rebuild any row"
    );
}
