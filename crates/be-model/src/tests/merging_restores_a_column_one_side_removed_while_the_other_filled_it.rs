use super::*;

#[test]
fn merging_restores_a_column_one_side_removed_while_the_other_filled_it() {
    let base = board();
    let (_, done, _) = ids(&base);
    let ours = edited(&base, [Change::remove(done)]);
    let (_, add) = Column::CARDS.insert(done, Anchor::End, &card("ship"));
    let theirs = edited(&base, [add]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(
        columns(&merged),
        owned(&[("Todo", &["write", "review"]), ("Done", &["ship"])])
    );
}
