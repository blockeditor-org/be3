use super::*;

#[test]
fn three_cards_move_in_one_change_and_undo_puts_them_back() {
    let mut document = Document::new(&Board {
        columns: [column("Todo", &["a", "b", "c", "d", "e"])]
            .into_iter()
            .collect(),
        ..Board::default()
    });
    let todo = document.root().columns[0].id;
    let before = document.clone();
    let shift = Column::CARDS.move_range(
        todo,
        card_id(&document, "b"),
        card_id(&document, "d"),
        Anchor::End,
    );

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

    assert_eq!(
        document.step(
            &Column::CARDS
                .move_range(
                    todo,
                    card_id(&document, "b"),
                    card_id(&document, "d"),
                    Anchor::After(card_id(&document, "c")),
                )
                .into()
        ),
        None,
        "a range cannot move inside itself"
    );
}
