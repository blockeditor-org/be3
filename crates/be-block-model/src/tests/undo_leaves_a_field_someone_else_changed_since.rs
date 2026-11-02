use super::*;

#[test]
fn undo_leaves_a_field_someone_else_changed_since() {
    let mut document = board();
    let (_, _, write) = ids(&document);

    let renamed = undone(
        &mut document,
        &Card::TEXT.set(write, &"draft".to_owned()).into(),
    );
    let checked = undone(&mut document, &Card::DONE.set(write, &true).into());
    document.apply(&Card::TEXT.set(write, &"final".to_owned()).into());

    document.apply(&renamed.undo());
    document.apply(&checked.undo());

    assert_eq!(
        document.read::<Card>(write),
        Some(Card {
            text: "final".to_owned(),
            done: false,
        })
    );
}
