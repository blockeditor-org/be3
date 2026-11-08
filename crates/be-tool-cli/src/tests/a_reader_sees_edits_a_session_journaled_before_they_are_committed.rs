use std::time::Duration;

use be_type_text::{TextBlock, TextContent};

use super::*;
use crate::replica::Session;

#[tokio::test]
async fn a_reader_sees_edits_a_session_journaled_before_they_are_committed() {
    tokio::task::LocalSet::new().run_until(read()).await;
}

async fn read() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let block = Uuid::now_v7();
    let peer = tracker.peer();
    peer.create_block(
        block,
        TextContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named("Notes"),
        false,
    )
    .await
    .unwrap();
    peer.save(block, &TextBlock::of("one\n"), None)
        .await
        .unwrap();
    let head = peer.remote_head(block).await.unwrap();

    let mut session =
        Session::<MemoryStore, TextContent>::open(tracker.shared_peer(), block, sources()())
            .await
            .unwrap();
    let journaled = async {
        while !session.is_owner() || session.content().is_none() {
            let woken = session.next().await;
            session.handle(woken).await.unwrap();
        }
        let client = session.client().unwrap();
        let edit = TextBlock::insert(session.content().unwrap(), client, 4, b"two\n").unwrap();
        session.edit(edit);
        session.settle().await;
        while !session
            .status()
            .is_some_and(|status| status.journaled && !status.clean)
        {
            let woken = session.next().await;
            session.handle(woken).await.unwrap();
        }
    };
    tokio::time::timeout(Duration::from_secs(60), journaled)
        .await
        .expect("the session journaled the edit");

    assert_eq!(
        peer.remote_head(block).await.unwrap(),
        head,
        "the edit waits in the journal for a pause before it is committed"
    );
    assert_eq!(
        tracker.text(block).await.unwrap(),
        "one\ntwo\n",
        "a reader outside the session replays the journal on the head"
    );
    session.close().await;
    harness.stop().await;
}
