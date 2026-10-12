use super::*;
use crate::{BlockMetadata, INTRINSIC_NAME, MetaKey, NAME, THUMBHASH};

#[test]
fn block_metadata_keeps_keys_it_does_not_know_and_reads_each_key_alone() {
    let unknown: MetaKey<Vec<String>> = MetaKey::defined(0x7465_7374, "someone else's");
    let mut metadata = BlockMetadata::default();
    metadata.insert(&NAME, &"Groceries".to_owned());
    metadata.insert(&INTRINSIC_NAME, &"milk".to_owned());
    metadata.insert(&unknown, &vec!["kept".to_owned()]);
    metadata
        .derived
        .insert(THUMBHASH.id, be_model::to_cbor(&"not a thumbhash"));

    let read = BlockMetadata::decode(&metadata.encode()).expect("the metadata decodes");

    assert_eq!(read, metadata);
    assert_eq!(read.get(&unknown), Some(vec!["kept".to_owned()]));
    assert_eq!(
        read.get(&THUMBHASH),
        None,
        "a value of the wrong shape reads as missing"
    );
    assert_eq!(read.name().as_deref(), Some("Groceries"));
    assert!(read.named_by_hand());

    let mut unnamed = read.clone();
    unnamed.remove(&NAME);
    assert_eq!(unnamed.name().as_deref(), Some("milk"));
    assert!(!unnamed.named_by_hand());
    assert!(BlockMetadata::decode(&[0xff, 0x00]).is_err());
    assert_eq!(BlockMetadata::decode(&[]), Ok(BlockMetadata::default()));
}
