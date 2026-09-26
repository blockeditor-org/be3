use super::*;

#[test]
fn edits_to_different_blocks_merge_cleanly() {
    let vault = vault();
    let (left, right) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (left, entry(Some(ROOT), manifest(&vault, "left"))),
        (right, entry(Some(ROOT), manifest(&vault, "right"))),
    ]);
    let mut ours = base.clone();
    ours.entries.get_mut(&left).unwrap().content = Some(manifest(&vault, "left, edited"));
    let mut theirs = base.clone();
    theirs.entries.get_mut(&right).unwrap().content = Some(manifest(&vault, "right, edited"));
    let added = Uuid::from_u128(4);
    theirs
        .entries
        .insert(added, entry(Some(ROOT), manifest(&vault, "new")));

    let plan = merge(&base, &ours, &theirs);

    assert!(plan.conflicts.is_empty(), "{:?}", plan.conflicts);
    assert!(plan.contents.is_empty());
    assert_eq!(plan.tree.entries[&left], ours.entries[&left]);
    assert_eq!(plan.tree.entries[&right], theirs.entries[&right]);
    assert!(plan.tree.entries.contains_key(&added));
}
