use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_column_removed_on_one_side_stays_removed_when_the_other_edits_a_card_in_it() {
    let base = board();
    let (todo, _, write) = ids(&base);
    let removed = edited(&base, [Change::remove(todo)]);
    let changed = edited(&base, [Card::TEXT.set(write, &"write it".to_owned())]);

    let (ours_removed, ours_conflicts) = Document::merge(&base, &removed, &changed);
    let (theirs_removed, theirs_conflicts) = Document::merge(&base, &changed, &removed);

    assert_eq!((ours_conflicts, theirs_conflicts), (1, 1));

    assert_eq!(columns(&ours_removed), owned(&[("Done", &[])]));
    assert_eq!(columns(&theirs_removed), owned(&[("Done", &[])]));
}
