use super::*;

#[test]
fn a_card_cannot_be_moved_into_a_list_of_columns_nor_a_column_among_cards() {
    let document = board();
    let (todo, done, write) = ids(&document);

    let card_among_columns = Board::COLUMNS.move_into(ObjectId::ROOT, Anchor::End, write);
    let column_among_cards = Column::CARDS.move_into(todo, Anchor::End, done);
    for edit in [card_among_columns, column_among_cards] {
        assert_eq!(document.step(&edit.clone().into()), None);
        assert_eq!(edited(&document, [edit]), document);
    }

    let moved = edited(
        &document,
        [Column::CARDS.move_into(done, Anchor::End, write)],
    );
    assert_eq!(
        columns(&moved),
        owned(&[("Todo", &["review"]), ("Done", &["write"])])
    );
    let mut reloaded = Document::<Board>::from_bytes(&moved.to_bytes()).expect("the bytes decode");
    reloaded
        .adopt_session_state(&moved.session_state())
        .expect("the session state is adopted");
    assert_eq!(reloaded, moved);
}
