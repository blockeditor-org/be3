use super::*;

#[test]
#[ignore = "an insert anchored after a removed sibling falls back to the end of the list"]
fn a_card_inserted_after_one_someone_removed_at_once_lands_where_that_one_was() {
    let base = board();
    let (todo, _, write) = ids(&base);
    let (_, insert) = Column::CARDS.insert(todo, Anchor::After(write), &card("draft"));

    let sequenced = edited(&base, [Change::remove(write), insert]);

    assert_eq!(
        columns(&sequenced),
        owned(&[("Todo", &["draft", "review"]), ("Done", &[])])
    );
}
