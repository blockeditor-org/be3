use super::*;

#[test]
#[ignore = "a move's undo moves the card back even when someone else has moved it since"]
fn undoing_a_move_leaves_a_card_someone_else_moved_since() {
    let mut document = board();
    let (todo, done, write) = ids(&document);

    let step = undone(
        &mut document,
        &Column::CARDS.move_into(done, Anchor::End, write).into(),
    );
    document.apply(&Column::CARDS.move_into(todo, Anchor::End, write).into());
    document.apply(&step.undo());

    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["review", "write"]), ("Done", &[])])
    );
}
