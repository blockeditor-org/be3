use super::*;

#[test]
fn undoing_an_edit_to_a_card_removed_since_changes_nothing() {
    let mut document = board();
    let (_, _, write) = ids(&document);

    let step = undone(
        &mut document,
        &Card::TEXT.set(write, &"draft".to_owned()).into(),
    );
    document.apply(&Change::remove(write).into());
    let removed = document.clone();

    document.apply(&step.undo());
    assert_eq!(document, removed);
    document.apply(&step.redo());
    assert_eq!(document, removed);
}
