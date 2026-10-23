use super::*;

use be_block::{INTRINSIC_NAME, NAME};

#[test]
fn both_sides_naming_a_block_is_a_name_conflict_and_derived_names_are_not() {
    let vault = vault();
    let note = Uuid::from_u128(2);
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (note, entry(Some(ROOT), manifest(&vault, "note"))),
    ]);
    let mut ours = base.clone();
    let mut theirs = base.clone();
    for (side, name) in [(&mut ours, "ours"), (&mut theirs, "theirs")] {
        side.entries
            .get_mut(&note)
            .unwrap()
            .metadata
            .insert(&INTRINSIC_NAME, &name.to_owned());
    }

    let plan = merge(&base, &ours, &theirs);
    assert!(plan.tree.conflicts.is_empty(), "{:?}", plan.tree.conflicts);

    ours.entries
        .get_mut(&note)
        .unwrap()
        .metadata
        .insert(&NAME, &"Our note".to_owned());
    theirs
        .entries
        .get_mut(&note)
        .unwrap()
        .metadata
        .insert(&NAME, &"Their note".to_owned());

    let plan = merge(&base, &ours, &theirs);
    assert_eq!(kinds(&plan.tree), vec![(note, ConflictKind::Name)]);

    let mut content = theirs.clone();
    content.entries.get_mut(&note).unwrap().metadata = ours.entries[&note].metadata.clone();
    content.entries.get_mut(&note).unwrap().content = Some(manifest(&vault, "edited"));
    content
        .entries
        .get_mut(&note)
        .unwrap()
        .metadata
        .insert(&INTRINSIC_NAME, &"edited".to_owned());
    let plan = merge(&base, &ours, &content);
    assert_eq!(
        plan.tree.entries[&note]
            .metadata
            .get(&INTRINSIC_NAME)
            .as_deref(),
        Some("edited"),
        "the derived name comes with the content it describes"
    );
}
