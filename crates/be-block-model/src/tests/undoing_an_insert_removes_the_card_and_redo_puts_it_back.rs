use super::*;

#[test]
fn undoing_an_insert_removes_the_card_and_redo_puts_it_back() {
    let mut document = board();
    let (todo, _, write) = ids(&document);
    let before = document.clone();
    let (_, insert) = Column::CARDS.insert(todo, Anchor::After(write), &card("draft"));

    let step = undone(&mut document, &insert.into());
    document.apply(&step.undo());
    assert_eq!(document, before);

    document.apply(&step.redo());
    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["write", "draft", "review"]), ("Done", &[])])
    );
}
