use super::*;

use be_block::{BlockContent, INTRINSIC_NAME, NAME};
use be_type_image::{ImageContent, ImageFile};

/// A name given offline to a block with no content waits in the outbox; content written after it
/// offline is sealed with that name and what the content says of itself, which is what this
/// device keeps of the block, not the name alone the waiting definition holds.
#[tokio::test]
async fn a_block_named_then_written_offline_keeps_the_metadata_it_was_sealed_with() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let block = laptop
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    offline
        .keep_definition(block, |metadata| {
            metadata.insert(&NAME, &"Holiday".to_owned());
        })
        .await
        .unwrap();
    let image = ImageFile::from_file("beach.png", vec![1, 2, 3]);
    let written = Write::encoded(ImageContent::CONTENT_TYPE, image.parts(), None)
        .described(Vec::new(), be_block::derived_metadata(&image));
    offline.seal(block, written).await.unwrap();

    let kept = offline.known_summary(block).unwrap();
    let kept = offline.metadata_of_summary(&kept).await.unwrap();
    assert_eq!(kept.get(&NAME).as_deref(), Some("Holiday"));
    assert_eq!(
        kept.get(&INTRINSIC_NAME).as_deref(),
        Some("beach.png"),
        "the waiting definition's metadata replaced what the content was sealed with"
    );
    harness.stop().await;
}
