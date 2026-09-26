use super::*;

#[test]
fn a_block_left_without_its_parent_is_moved_under_the_root() {
    let vault = vault();
    let (folder, note) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "root"))),
        (folder, entry(Some(ROOT), manifest(&vault, "folder"))),
    ]);
    let mut ours = base.clone();
    ours.entries.remove(&folder);
    let mut theirs = base.clone();
    theirs
        .entries
        .insert(note, entry(Some(folder), manifest(&vault, "note")));

    let plan = merge(&base, &ours, &theirs);

    assert!(!plan.tree.entries.contains_key(&folder));
    assert_eq!(plan.tree.entries[&note].parent, Some(ROOT));
    assert_eq!(
        plan.conflicts,
        vec![Conflict {
            block: note,
            kind: ConflictKind::Placement,
        }]
    );
}
