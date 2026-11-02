use super::*;

#[test]
fn an_insert_reusing_an_id_already_in_the_document_is_refused() {
    let document = board();
    let (_, done, write) = ids(&document);

    let unchanged = edited(
        &document,
        [Column::CARDS.insert_as(write, done, Anchor::End, &card("again"))],
    );

    assert_eq!(unchanged, document);
}
