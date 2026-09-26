use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_column_removed_on_one_side_stays_removed_when_the_other_moves_a_card_into_it() {
    let base = board();
    let (_, done, write) = ids(&base);
    let removed = edited(&base, [Change::remove(done)]);
    let moved = edited(&base, [Column::CARDS.move_into(done, Anchor::End, write)]);

    let (ours_removed, ours_conflicts) = Document::merge(&base, &removed, &moved);
    let (theirs_removed, theirs_conflicts) = Document::merge(&base, &moved, &removed);

    assert_eq!((ours_conflicts, theirs_conflicts), (1, 1));

    let expected = owned(&[("Todo", &["write", "review"])]);
    assert_eq!(columns(&ours_removed), expected);
    assert_eq!(columns(&theirs_removed), expected);
}
