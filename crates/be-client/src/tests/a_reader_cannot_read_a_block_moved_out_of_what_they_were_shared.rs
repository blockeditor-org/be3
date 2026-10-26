use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_reader_cannot_read_a_block_moved_out_of_what_they_were_shared() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let reader = harness.member(&owner, "reader@example.com").await;
    let team = workspace(&owner, "Team").await;
    let private = workspace(&owner, "Private").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let before = owner
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let public = owner
        .keys()
        .contact(reader.account())
        .expect("the invitation made them contacts");
    owner
        .share(team, reader.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&reader, note).await, "before");
    let old_key = owner.summary(note).await.unwrap().key.unwrap();

    owner
        .set_parent(note, BlockParent::Block(private))
        .await
        .unwrap();
    assert_eq!(owner.key_work().await.unwrap().rotations, [note]);
    owner.tend().await.unwrap();
    assert!(owner.key_work().await.unwrap().rotations.is_empty());
    let new_key = owner.summary(note).await.unwrap().key.unwrap();
    assert_ne!(
        new_key, old_key,
        "the moved block kept writing with the old key"
    );
    let after = owner
        .save(note, &TextBlock::of("after"), Some(before))
        .await
        .unwrap()
        .published()
        .unwrap();
    assert_eq!(text(&owner, note).await, "after");

    assert!(reader.summary(note).await.is_err());
    reader.refresh_keys(Some(vec![note, team])).await.unwrap();
    assert!(
        !reader.keys().has(new_key),
        "the reader got the moved block's new key"
    );
    assert!(
        !reader.try_fetch(after.hash()).await.unwrap(),
        "the server served the reader a commit written after the move"
    );
    harness.stop().await;
}
