use super::*;

#[test]
fn a_block_edited_on_one_side_and_removed_on_the_other_is_kept_as_a_conflict() {
    let vault = vault();
    let (edited, dropped) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (edited, entry(Some(ROOT), manifest(&vault, "edited"))),
        (dropped, entry(Some(ROOT), manifest(&vault, "dropped"))),
    ]);
    let mut ours = base.clone();
    ours.entries.get_mut(&edited).unwrap().content = Some(manifest(&vault, "edited, more"));
    ours.entries.remove(&dropped);
    let mut theirs = base.clone();
    theirs.entries.remove(&edited);

    let plan = merge(&base, &ours, &theirs);

    assert_eq!(plan.tree.entries[&edited], ours.entries[&edited]);
    assert!(!plan.tree.entries.contains_key(&dropped));
    assert_eq!(
        plan.conflicts,
        vec![Conflict {
            block: edited,
            kind: ConflictKind::EditedAndRemoved,
        }]
    );
}
