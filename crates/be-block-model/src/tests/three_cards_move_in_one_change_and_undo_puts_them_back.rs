use super::*;

#[test]
fn three_cards_move_in_one_change_and_undo_puts_them_back() {
    let (mut document, todo) = todo_column(&["a", "b", "c", "d", "e"]);
    let before = document.clone();
    let shift = Column::CARDS.move_range(todo, card_ids(&document, &["b", "c", "d"]), Anchor::End);

    let step = undone(&mut document, &shift.into());
    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["a", "e", "b", "c", "d"])])
    );
    let after = document.clone();

    document.apply(&step.undo());
    assert_eq!(document, before, "undo puts the three cards back");
    document.apply(&step.redo());
    assert_eq!(document, after);

    let refused = |document: &Document<Board>, texts: &[&str], anchor: Anchor| {
        document
            .step(
                &Column::CARDS
                    .move_range(todo, card_ids(document, texts), anchor)
                    .into(),
            )
            .is_none()
    };
    assert!(
        refused(
            &document,
            &["b", "c", "d"],
            Anchor::After(card_id(&document, "c"))
        ),
        "a range cannot move inside itself"
    );
    assert!(
        refused(&document, &["b", "c", "d"], Anchor::End),
        "the range is already last"
    );
    assert!(
        refused(&document, &["b", "d"], Anchor::Start),
        "the cards are not shown together"
    );
    assert!(
        refused(&document, &["c", "b"], Anchor::Start),
        "the cards are not shown in this order"
    );
    assert!(
        refused(&document, &["b"], Anchor::Start),
        "one card moves with `move_into`"
    );
}
