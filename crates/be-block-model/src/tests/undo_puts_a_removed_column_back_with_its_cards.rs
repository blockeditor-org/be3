use super::*;

#[test]
fn undo_puts_a_removed_column_back_with_its_cards() {
    let mut document = board();
    let original = document.clone();
    let (todo, _, _) = ids(&document);

    let step = undone(&mut document, &Change::remove(todo).into());
    assert_eq!(columns(&document), owned(&[("Done", &[])]));

    document.apply(&step.undo());
    assert_eq!(document, original);
    document.apply(&step.redo());
    assert_eq!(columns(&document), owned(&[("Done", &[])]));
}
