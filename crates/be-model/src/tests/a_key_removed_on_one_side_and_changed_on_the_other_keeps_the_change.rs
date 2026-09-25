use super::*;

#[test]
#[ignore = "a map key removed on our side and changed on theirs takes our removal and loses their change"]
fn a_key_removed_on_one_side_and_changed_on_the_other_keeps_the_change() {
    let base = sheet(&[(1, "one")]);
    let removed = put(&base, &[(1, None)]);
    let changed = put(&base, &[(1, Some("uno"))]);

    let (ours_removed, ours_conflicts) = Document::merge(&base, &removed, &changed);
    let (theirs_removed, theirs_conflicts) = Document::merge(&base, &changed, &removed);

    assert_eq!((ours_conflicts, theirs_conflicts), (1, 1));
    assert_eq!(cell(&ours_removed, 1).as_deref(), Some("uno"));
    assert_eq!(cell(&theirs_removed, 1).as_deref(), Some("uno"));
}
