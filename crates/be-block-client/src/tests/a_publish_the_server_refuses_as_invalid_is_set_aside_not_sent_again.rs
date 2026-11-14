use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::*;

#[tokio::test]
async fn a_publish_the_server_refuses_as_invalid_is_set_aside_not_sent_again() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let files = my_files(&owner).await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(files))
        .await
        .unwrap();
    let base = owner
        .save(note, &TextBlock::of("kept"), None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    assert_eq!(text(&laptop, note).await, "kept");
    drop(laptop);

    // A name the server's metadata limit refuses, carried by the commit saved after it.
    let offline = harness.offline(&owner, &refs, &store);
    let _ = offline
        .define(note, |metadata| {
            *metadata = be_block_content::BlockMetadata::named("n".repeat(70 * 1024));
        })
        .await;
    let _ = offline
        .save(note, &TextBlock::of("written while away"), Some(base))
        .await;

    harness.reconnect(&offline, owner.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block: note,
            code: ErrorCode::InvalidRequest
        }),
        "{drained:?}"
    );
    assert_eq!(
        offline.pending().operations,
        0,
        "the refused publish still waits in the outbox, to be sent again every round"
    );
    assert!(
        offline.unsent().iter().any(|unsent| unsent.block == note),
        "the refused publish was dropped rather than set aside among the unsent edits"
    );
    assert!(
        offline.drain(|_| false).await.unwrap().is_empty(),
        "a second drain sent the refused publish again"
    );
    assert_eq!(text(&owner, note).await, "kept");

    harness.stop().await;
}
