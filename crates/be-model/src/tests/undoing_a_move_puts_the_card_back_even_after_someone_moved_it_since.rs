use super::*;

#[test]
fn undoing_a_move_puts_the_card_back_even_after_someone_moved_it_since() {
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
        owned(&[("Todo", &["write", "review"]), ("Done", &[])])
    );
}
