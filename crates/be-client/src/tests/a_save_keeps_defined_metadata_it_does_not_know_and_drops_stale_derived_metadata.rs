use super::*;

use be_block::{BlockMetadata, INTRINSIC_NAME, MetaKey, NAME};
use image_block_types::{ImageContent, ImageFile};

const PINNED: MetaKey<bool> = MetaKey::defined(0x7069_6e6e_6564, "pinned");
const WORDS: MetaKey<u32> = MetaKey::derived(0x0077_6f72_6473, "words");

#[tokio::test]
async fn a_save_keeps_defined_metadata_it_does_not_know_and_drops_stale_derived_metadata() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    let first = phone
        .save(
            block,
            &ImageFile::from_file("beach.png", vec![1, 2, 3]),
            None,
        )
        .await
        .unwrap()
        .published()
        .unwrap();
    let named = phone.metadata_of(first).await.unwrap();
    assert_eq!(named.get(&INTRINSIC_NAME).as_deref(), Some("beach.png"));

    let mut newer = named.clone();
    newer.insert(&PINNED, &true);
    newer.insert(&WORDS, &3);
    let commit = phone.load_commit(first).await.unwrap();
    let second = phone
        .write(
            block,
            Write::written(commit.manifest, Some(first))
                .described(commit.references, newer.derived)
                .defined(newer.defined),
        )
        .await
        .unwrap()
        .published()
        .unwrap();

    let renamed = phone
        .define(block, |metadata| {
            metadata.insert(&NAME, &"Holiday".to_owned());
        })
        .await
        .unwrap();
    let renamed = phone.metadata_of_summary(&renamed).await.unwrap();
    assert_eq!(
        renamed.get(&WORDS),
        Some(3),
        "a rename leaves the content's metadata alone"
    );
    assert_eq!(renamed.get(&PINNED), Some(true));
    let head = phone.remote_head(block).await.unwrap();
    assert_ne!(head, Some(second));

    let third = phone
        .save(block, &ImageFile::from_file("sand.png", vec![4, 5]), head)
        .await
        .unwrap()
        .published()
        .unwrap();
    let saved = phone.metadata_of(third).await.unwrap();
    assert_eq!(
        saved.get(&PINNED),
        Some(true),
        "defined metadata outlives the save"
    );
    assert_eq!(saved.get(&NAME).as_deref(), Some("Holiday"));
    assert_eq!(saved.get(&INTRINSIC_NAME).as_deref(), Some("sand.png"));
    assert_eq!(
        saved.get(&WORDS),
        None,
        "derived metadata nobody here can recompute is dropped"
    );
    let summary = phone.summary(block).await.unwrap();
    assert_eq!(phone.metadata_of_summary(&summary).await.unwrap(), saved);
    assert_ne!(saved, BlockMetadata::default());

    harness.stop().await;
}
