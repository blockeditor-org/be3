use super::*;
use workspace_ui_types::WorkspaceContent;

use be_block::BlockMetadata;
use be_protocol::WrappedBy;
use text_block_types::{TextBlock, TextContent};

async fn workspace(peer: &Peer<MemoryStore>, name: &str) -> Uuid {
    peer.create_block(
        Uuid::now_v7(),
        WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named(name),
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn a_block_moved_to_another_workspace_is_rewrapped_and_given_a_fresh_key() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let mine = workspace(&phone, "My files").await;
    let team = workspace(&phone, "Team").await;
    let note = phone
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    phone
        .save(note, &TextBlock::of("before the move"), None)
        .await
        .unwrap();
    let before = phone.summary(note).await.unwrap();
    let old_key = before.key.unwrap();
    assert_eq!(before.scope, Some(mine));
    let mine_scope = phone.keys().scope_key(mine).unwrap();
    let team_scope = phone.keys().scope_key(team).unwrap();
    assert_eq!(
        phone.keys().wrapped_by(old_key),
        [WrappedBy::Key(mine_scope)]
    );

    phone
        .set_parent(note, BlockParent::Block(team))
        .await
        .unwrap();
    let after = phone.summary(note).await.unwrap();
    assert_eq!(after.scope, Some(team));
    let fresh = after.key.unwrap();
    assert_ne!(
        fresh, old_key,
        "the moved block kept writing with its old key"
    );

    let laptop = harness.second(&phone).await;
    assert_eq!(
        laptop.keys().wrapped_by(old_key),
        [WrappedBy::Key(team_scope)],
        "the old key is still wrapped under the workspace the block left"
    );
    assert_eq!(
        laptop.keys().wrapped_by(fresh),
        [WrappedBy::Key(team_scope)]
    );
    let opened = laptop.open::<TextContent>(note).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&opened), "before the move");
    let head = laptop.local_head(note);
    laptop
        .save(note, &TextBlock::of("after the move"), head)
        .await
        .unwrap();
    let read = phone.open::<TextContent>(note).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&read), "after the move");

    harness.stop().await;
}
