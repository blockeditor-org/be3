use be_type_text::TextContent;

use super::*;

/// What is made in a block the server refused to make cannot be made either: it is set aside
/// with the parent's refusal, not taken for lost access, and restoring the parent restores it.
#[tokio::test]
async fn what_is_made_in_a_block_the_server_refused_waits_aside_with_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let (parent, child) = (Uuid::now_v7(), Uuid::now_v7());
    owner
        .keep_creation(
            parent,
            be_type_workspace::FolderContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("n".repeat(70 * 1024)),
            false,
        )
        .await
        .unwrap();
    owner
        .keep_creation(
            child,
            TextContent::CONTENT_TYPE,
            BlockParent::Block(parent),
            &be_block_content::BlockMetadata::named("Note"),
            false,
        )
        .await
        .unwrap();

    let drained = owner.drain(|_| false).await.unwrap();
    let refused = be_block_protocol::ErrorCode::InvalidRequest;
    assert!(
        drained.contains(&Drained::SetAside {
            block: child,
            code: refused
        }),
        "{drained:?}"
    );
    let unsent = owner.unsent();
    let row = unsent
        .iter()
        .find(|unsent| unsent.block == child)
        .expect("the child is set aside");
    assert_eq!(row.refused, Some(refused));
    assert_eq!(owner.pending().operations, 0);

    assert_eq!(owner.restore_unsent(parent).unwrap(), vec![parent, child]);
    assert!(owner.creating(parent) && owner.creating(child));

    harness.stop().await;
}
