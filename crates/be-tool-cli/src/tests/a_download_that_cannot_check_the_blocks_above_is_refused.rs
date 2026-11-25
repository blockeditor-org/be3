use be_type_database::database_view::DatabaseViewContent;

use super::*;

/// Whether a block lies in one only its owner reads is asked of each block above it: a device
/// that cannot reach the server and has not heard of the block above exports nothing, rather
/// than taking an unanswered question for a block that is not private.
#[tokio::test]
async fn a_download_that_cannot_check_the_blocks_above_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner().await;
    let settings = Uuid::now_v7();
    owner
        .create_block(
            settings,
            be_block_graph::SETTINGS_TYPE,
            BlockParent::Root,
            &BlockMetadata::default(),
            false,
        )
        .await
        .unwrap();
    let view = Uuid::now_v7();
    owner
        .create_block(
            view,
            DatabaseViewContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &BlockMetadata::named("Private"),
            true,
        )
        .await
        .unwrap();
    let database = Uuid::now_v7();
    owner
        .create_block(
            database,
            DatabaseContent::CONTENT_TYPE,
            BlockParent::Block(view),
            &BlockMetadata::named("Private"),
            false,
        )
        .await
        .unwrap();
    // Another device of the owner's, through a relay, which reads the database and what lies in
    // it, and nothing above, before the network goes down: all an export needs but the check.
    let relay = be_block_fault_relay::Relay::start(harness.address);
    let device = Peer::connect(
        PeerConfig::new(
            be_block_client::socket_url(&relay.url()),
            owner.keys().identity().clone(),
            Credentials::Token(owner.token()),
        )
        .organization(Some(owner.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let summary = device.summary(database).await.unwrap();
    device.children(BlockParent::Block(database)).await.unwrap();
    device.learn(std::slice::from_ref(&summary)).await.unwrap();
    device
        .read_contents(&[(database, summary.head)])
        .await
        .unwrap();
    let tracker = Tracker::new(device, sources());
    relay.offline(true);

    let collected = be_block_type_host::transfer::collect(
        tracker.peer(),
        database,
        be_host_stack::discovery::exported_whole,
        &mut *tracker.types().await,
    )
    .await;
    match collected {
        Err(error) => assert!(error.is_unreachable(), "{error}"),
        Ok(blocks) => panic!("{} blocks were exported", blocks.len()),
    }
    harness.stop().await;
}
