use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_key_removed_on_one_side_and_changed_on_the_other_stays_removed() {
    let base = sheet(&[(1, "one")]);
    let removed = put(&base, &[(1, None)]);
    let changed = put(&base, &[(1, Some("uno"))]);

    let (ours_removed, ours_conflicts) = Document::merge(&base, &removed, &changed);
    let (theirs_removed, theirs_conflicts) = Document::merge(&base, &changed, &removed);

    assert_eq!((ours_conflicts, theirs_conflicts), (1, 1));

    assert_eq!(cell(&ours_removed, 1), None);
    assert_eq!(cell(&theirs_removed, 1), None);
}
