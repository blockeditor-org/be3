use super::*;

#[test]
fn a_card_moved_to_different_columns_on_each_side_conflicts_and_goes_where_ours_put_it() {
    let base = board();
    let (todo, done, write) = ids(&base);
    let (_, add_later) = Board::COLUMNS.insert(ObjectId::ROOT, Anchor::End, &column("Later", &[]));
    let base = edited(&base, [add_later]);
    let later = base.root().columns[2].id;
    let ours = edited(&base, [Column::CARDS.move_into(done, Anchor::End, write)]);
    let theirs = edited(&base, [Column::CARDS.move_into(later, Anchor::End, write)]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(
        columns(&merged),
        owned(&[("Todo", &["review"]), ("Done", &["write"]), ("Later", &[])])
    );
    assert_eq!(merged.ids(todo, Column::CARDS).len(), 1);
}
