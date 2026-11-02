use super::*;

#[test]
fn undoing_a_move_puts_the_card_back_where_it_was() {
    let mut document = board();
    let (_, done, _) = ids(&document);
    let review = card_id(&document, "review");
    let before = document.clone();

    let step = undone(
        &mut document,
        &Column::CARDS.move_into(done, Anchor::End, review).into(),
    );
    document.apply(&step.undo());
    assert_eq!(document, before);

    document.apply(&step.redo());
    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["write"]), ("Done", &["review"])])
    );
}
