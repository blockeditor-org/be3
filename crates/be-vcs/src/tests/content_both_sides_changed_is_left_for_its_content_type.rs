use super::*;

#[test]
fn content_both_sides_changed_is_left_for_its_content_type() {
    let vault = vault();
    let note = Uuid::from_u128(2);
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (note, entry(Some(ROOT), manifest(&vault, "note"))),
    ]);
    let mut ours = base.clone();
    ours.entries.get_mut(&note).unwrap().content = Some(manifest(&vault, "ours"));
    let mut theirs = base.clone();
    theirs.entries.get_mut(&note).unwrap().content = Some(manifest(&vault, "theirs"));
    let moved = theirs.entries.get_mut(&note).unwrap();
    moved.metadata = BlockMetadata::named("Renamed");

    let plan = merge(&base, &ours, &theirs);

    assert!(plan.conflicts.is_empty(), "{:?}", plan.conflicts);
    assert_eq!(
        plan.contents,
        vec![ContentMerge {
            block: note,
            content_type: CONTENT,
            base: base.entries[&note].content.clone(),
            ours: ours.entries[&note].content.clone(),
            theirs: theirs.entries[&note].content.clone(),
        }]
    );
    assert_eq!(
        plan.tree.entries[&note].metadata,
        BlockMetadata::named("Renamed")
    );
}
