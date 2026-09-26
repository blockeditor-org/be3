use super::*;

#[test]
fn an_insert_into_a_removed_column_changes_nothing() {
    let document = board();
    let (_, done, _) = ids(&document);
    let removed = edited(&document, [Change::remove(done)]);

    let (card, insert) = Column::CARDS.insert(done, Anchor::End, &card("ship"));
    let inserted = edited(&removed, [insert]);

    assert_eq!(inserted, removed);
    assert_eq!(inserted.read::<Card>(card), None);
}
