use super::*;
use crate::{ARTIFACT, Artifact, BlockMetadata, LOCAL_ID, NAME};

#[test]
fn defined_metadata_merges_key_by_key() {
    let mut base = BlockMetadata::named("Plan");
    base.insert(&LOCAL_ID, &Uuid::from_u128(1));
    let mut ours = base.clone();
    ours.insert(&NAME, &"Our plan".to_owned());
    let mut theirs = base.clone();
    theirs.remove(&LOCAL_ID);
    theirs.insert(
        &ARTIFACT,
        &Artifact {
            source: Uuid::from_u128(4).into(),
            source_type: Uuid::from_u128(2),
            settings: vec![3],
        },
    );

    let (merged, conflicts) = BlockMetadata::merge_defined(&base, &ours, &theirs);
    let merged = BlockMetadata {
        defined: merged,
        ..BlockMetadata::default()
    };

    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged.get(&NAME).as_deref(), Some("Our plan"));
    assert_eq!(merged.get(&LOCAL_ID), None);
    assert_eq!(
        merged.get(&ARTIFACT).map(|artifact| artifact.source_type),
        Some(Uuid::from_u128(2))
    );

    theirs.insert(&NAME, &"Their plan".to_owned());
    let (merged, conflicts) = BlockMetadata::merge_defined(&base, &ours, &theirs);
    assert_eq!(conflicts, vec![NAME.id]);
    assert_eq!(
        merged.get(&NAME.id),
        ours.defined.get(&NAME.id),
        "ours stays until it is resolved"
    );
}
