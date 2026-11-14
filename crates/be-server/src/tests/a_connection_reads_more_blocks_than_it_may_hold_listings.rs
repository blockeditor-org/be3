use super::*;
use crate::limits::MAX_LISTINGS_PER_CONNECTION;

/// A long-lived client reads every block it shows and never unlists what it read, so reading
/// blocks never runs a connection out of listings, and its folders still list after.
#[tokio::test]
async fn a_connection_reads_more_blocks_than_it_may_hold_listings() {
    let hub = WatchHub::new();
    let client = 1;
    for _ in 0..=MAX_LISTINGS_PER_CONNECTION {
        let block = Uuid::new_v4();
        let read = Listing::Block(block);
        let new = hub
            .list(client, read)
            .await
            .expect("reading a block was refused");
        assert!(new);
        let summary = BlockSummary::no_access(block, CONTENT, be_block_graph::Access::KnowExists);
        hub.listed(
            client,
            read,
            vec![(block, crate::watch::Facts::of(&summary, false))],
        )
        .await;
    }
    hub.list(client, Listing::Children(BlockParent::Root))
        .await
        .expect("a folder no longer lists after many blocks were read");
}
