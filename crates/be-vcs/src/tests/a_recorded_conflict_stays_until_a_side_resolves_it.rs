use super::*;

use be_block::NAME;

#[test]
fn a_recorded_conflict_stays_until_a_side_resolves_it() {
    let vault = vault();
    let (note, other) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let start = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (note, entry(Some(ROOT), manifest(&vault, "note"))),
        (other, entry(Some(ROOT), manifest(&vault, "other"))),
    ]);
    let mut named = [start.clone(), start.clone()];
    for (side, name) in named.iter_mut().zip(["Ours", "Theirs"]) {
        side.entries
            .get_mut(&note)
            .unwrap()
            .metadata
            .insert(&NAME, &name.to_owned());
    }
    let conflicted = merge(&start, &named[0], &named[1]).tree;
    assert_eq!(kinds(&conflicted), vec![(note, ConflictKind::Name)]);

    let mut edited = conflicted.clone();
    edited.entries.get_mut(&other).unwrap().content = Some(manifest(&vault, "other, edited"));
    let carried = merge(&start, &conflicted, &edited).tree;
    assert_eq!(
        kinds(&carried),
        vec![(note, ConflictKind::Name)],
        "a change that only touches another block keeps the conflict"
    );

    let mut resolved = conflicted.clone();
    resolved.resolve(note, Some(Side::Ours));
    let plan = merge(&conflicted, &edited, &resolved);
    assert!(plan.tree.conflicts.is_empty(), "{:?}", plan.tree.conflicts);
    assert_eq!(plan.tree.entries[&note], named[0].entries[&note]);
}
