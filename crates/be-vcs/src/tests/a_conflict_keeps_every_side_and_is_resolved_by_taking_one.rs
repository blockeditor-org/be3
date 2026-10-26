use super::*;

use be_block::NAME;

#[test]
fn a_conflict_keeps_every_side_and_is_resolved_by_taking_one() {
    let vault = vault();
    let note = Uuid::from_u128(2);
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (note, entry(Some(ROOT), manifest(&vault, "note"))),
    ]);
    let mut ours = base.clone();
    let mut theirs = base.clone();
    for (side, name) in [(&mut ours, "Ours"), (&mut theirs, "Theirs")] {
        side.entries
            .get_mut(&note)
            .unwrap()
            .metadata
            .insert(&NAME, &name.to_owned());
    }

    let mut merged = merge(&base, &ours, &theirs).tree;
    let conflict = &merged.conflicts[&note];
    assert_eq!(conflict.base.as_ref(), base.entries.get(&note));
    assert_eq!(conflict.ours.as_ref(), ours.entries.get(&note));
    assert_eq!(conflict.theirs.as_ref(), theirs.entries.get(&note));
    assert!(
        merged.objects().is_superset(&base.objects()),
        "the sides' contents are held with the tree"
    );

    assert!(merged.resolve(note, Some(Side::Theirs)));
    assert_eq!(merged.entries[&note], theirs.entries[&note]);
    assert!(merged.conflicts.is_empty());
    assert!(!merged.resolve(note, None), "it is already resolved");
}
