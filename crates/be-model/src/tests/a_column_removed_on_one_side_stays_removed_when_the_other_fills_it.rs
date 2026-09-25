use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_column_removed_on_one_side_stays_removed_when_the_other_fills_it() {
    let base = board();
    let (_, done, _) = ids(&base);
    let removed = edited(&base, [Change::remove(done)]);
    let (_, add) = Column::CARDS.insert(done, Anchor::End, &card("ship"));
    let filled = edited(&base, [add]);

    let (ours_removed, ours_conflicts) = Document::merge(&base, &removed, &filled);
    let (theirs_removed, theirs_conflicts) = Document::merge(&base, &filled, &removed);

    assert_eq!((ours_conflicts, theirs_conflicts), (1, 1));

    let expected = owned(&[("Todo", &["write", "review"])]);
    assert_eq!(columns(&ours_removed), expected);
    assert_eq!(columns(&theirs_removed), expected);
}
