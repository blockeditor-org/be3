use be_block_protocol::ChangeCursor;

use super::*;

/// A page of `changed-since` that stops on a change it may not name reads on through that
/// change's sequence only until the next block it may name, which ends the page, rather than
/// taking the rest of the sequence however large.
#[tokio::test]
async fn a_page_that_reads_on_through_another_trees_change_stops_at_its_own_next_block() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.organization("notes").await;

    let work = owner.create_block(BlockParent::Root).await;
    let other = owner.create_block(BlockParent::Root).await;
    let moving = owner.create_block(BlockParent::Block(work)).await;
    let mut below = Vec::new();
    for _ in 0..20 {
        below.push(owner.create_block(BlockParent::Block(moving)).await);
    }
    let start = owner
        .call(|peer| {
            client::blocks::changed_since(
                peer,
                work,
                ChangeCursor {
                    changed: 0,
                    block: Uuid::max(),
                },
                1000,
            )
        })
        .await
        .expect("changed-since failed");
    let mut after = ChangeCursor {
        changed: start.sequence,
        block: Uuid::max(),
    };
    // One change of another tree's alone, then one move that changes 21 blocks of another tree
    // now and names each as having left this one: the page reads on through the move.
    let elsewhere = owner.create_block(BlockParent::Block(other)).await;
    let moved = owner.move_to(moving, BlockParent::Block(other)).await;
    assert!(moved.is_ok(), "{moved:?}");

    let mut left = Vec::new();
    let mut pages = 0;
    loop {
        let page = owner
            .call(|peer| client::blocks::changed_since(peer, work, after.clone(), 1000))
            .await
            .expect("changed-since failed");
        pages += 1;
        assert!(
            page.removed.len() <= 4,
            "a page took {} blocks past where it should have stopped",
            page.removed.len()
        );
        assert!(!page.removed.contains(&elsewhere));
        left.extend(page.removed);
        match page.next {
            Some(next) => after = next,
            None => break,
        }
        assert!(pages < 100, "the pages never ended");
    }
    let mut expected = below.clone();
    expected.push(moving);
    expected.sort();
    left.sort();
    assert_eq!(left, expected, "every block that left is named once");

    harness.stop().await;
}
