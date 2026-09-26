use super::*;

#[test]
fn the_same_value_put_under_a_key_on_both_sides_merges_without_a_conflict() {
    let base = sheet(&[(1, "one")]);
    let ours = put(&base, &[(1, Some("uno")), (2, None)]);
    let theirs = put(&base, &[(1, Some("uno")), (3, Some("three"))]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(cell(&merged, 1).as_deref(), Some("uno"));
    assert_eq!(cell(&merged, 3).as_deref(), Some("three"));
}
