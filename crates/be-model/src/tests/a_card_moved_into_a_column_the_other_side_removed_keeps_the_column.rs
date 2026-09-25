use super::*;

#[test]
fn a_card_moved_into_a_column_the_other_side_removed_keeps_the_column() {
    let base = board();
    let (_, done, write) = ids(&base);
    let ours = edited(&base, [Change::remove(done)]);
    let theirs = edited(&base, [Column::CARDS.move_into(done, Anchor::End, write)]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert!(conflicts >= 1);
    assert_eq!(
        columns(&merged),
        owned(&[("Todo", &["review"]), ("Done", &["write"])])
    );
}
