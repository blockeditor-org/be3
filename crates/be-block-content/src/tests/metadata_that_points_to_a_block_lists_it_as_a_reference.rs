use super::*;
use crate::{ARTIFACT, Artifact, BlockMetadata, NAME};

#[test]
fn metadata_that_points_to_a_block_lists_it_as_a_reference() {
    let source = Uuid::from_u128(7);
    let mut metadata = BlockMetadata::named("Export");
    assert!(metadata.references().is_empty());
    metadata.insert(
        &ARTIFACT,
        &Artifact {
            source: source.into(),
            source_type: Uuid::from_u128(2),
            settings: source.as_bytes().to_vec(),
        },
    );
    metadata.insert(&NAME, &source.to_string());

    assert_eq!(metadata.references(), vec![source]);
    let decoded = BlockMetadata::decode(&metadata.encode()).unwrap();
    assert_eq!(decoded.references(), vec![source]);
}
