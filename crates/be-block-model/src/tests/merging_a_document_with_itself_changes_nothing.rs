use super::*;

#[test]
fn merging_a_document_with_itself_changes_nothing() {
    let base = board();
    let (todo, done, write) = ids(&base);
    let (_, add) = Column::CARDS.insert(done, Anchor::End, &card("ship"));
    let changed = edited(
        &base,
        [
            add,
            Column::CARDS.move_into(done, Anchor::Start, write),
            Board::VOTES.add(ObjectId::ROOT, 3),
            Column::NAME.set(todo, &"Later".to_owned()),
        ],
    );

    assert_eq!(
        Document::merge(&base, &changed, &changed),
        (changed.clone(), 0)
    );
    assert_eq!(
        Document::merge(&changed, &changed, &changed),
        (changed.clone(), 0)
    );
    assert_eq!(Document::merge(&base, &base, &base), (base, 0));
}
