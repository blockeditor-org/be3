use be_block_protocol::ChangeCursor;

use super::*;

/// Every page after `since` in `root`'s tree: the blocks changed, the blocks that left, and the
/// sequence to continue from.
async fn changes(
    client: &mut TestClient,
    root: Uuid,
    since: u64,
    limit: u32,
) -> (Vec<Uuid>, Vec<Uuid>, u64) {
    let (mut changed, mut left) = (Vec::new(), Vec::new());
    let mut after = ChangeCursor {
        changed: since,
        block: Uuid::max(),
    };
    // A page looks at only so many changes in tests (`MAX_SCANNED`), and trashed blocks stay.
    for _ in 0..1_000 {
        let page = client
            .call(|peer| {
                peer.ask::<be_block_protocol::ChangedPage>(
                    be_block_protocol::Query::Tree {
                        entry: root,
                        after: after,
                        limit,
                    },
                    false,
                )
            })
            .await
            .expect("changed-since failed");
        changed.extend(page.summaries.iter().map(|summary| summary.id));
        left.extend(page.removed);
        match page.next {
            Some(next) => after = next,
            None => {
                changed.sort();
                left.sort();
                return (changed, left, page.sequence);
            }
        }
    }
    panic!("the pages never ended");
}

fn sorted(mut blocks: Vec<Uuid>) -> Vec<Uuid> {
    blocks.sort();
    blocks
}

#[tokio::test]
async fn changed_since_pages_what_changed_in_a_tree_and_names_what_left_it() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.organization("notes").await;

    let work = owner.create_block(BlockParent::Root).await;
    let other = owner.create_block(BlockParent::Root).await;
    let first = owner.create_block(BlockParent::Block(work)).await;
    let inner = owner.create_block(BlockParent::Block(first)).await;
    let stays = owner.create_block(BlockParent::Block(work)).await;
    let elsewhere = owner.create_block(BlockParent::Block(other)).await;
    let lone = owner.create_block(BlockParent::Root).await;

    // A block no block refers to is not held: the live ones are
    // referred to.
    for (parent, children) in [
        (work, vec![first, stays]),
        (first, vec![inner]),
        (other, vec![elsewhere]),
    ] {
        let referred = owner
            .call(|peer| {
                peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::Publish {
                    block: parent,
                    expected: None,
                    commit: None,
                    references: be_block_protocol::ReferenceChange::between(&[], &children),
                    metadata: Vec::new(),
                    journal: None,
                })
            })
            .await;
        assert!(referred.is_ok(), "{referred:?}");
    }

    // From the start, the tree's blocks, however the pages are cut, and none of `other`'s.
    let tree = sorted(vec![work, first, inner, stays]);
    for limit in [1, 2, 100] {
        let (changed, left, _) = changes(&mut owner, work, 0, limit).await;
        assert_eq!(changed, tree, "limit {limit}");
        assert!(left.is_empty(), "nothing has left yet: {left:?}");
    }
    let (_, _, watermark) = changes(&mut owner, work, 0, 100).await;

    // Nothing changed since: nothing is listed, and the sequence has not moved.
    let (changed, left, again) = changes(&mut owner, work, watermark, 100).await;
    assert!(
        changed.is_empty() && left.is_empty(),
        "{changed:?} {left:?}"
    );
    assert_eq!(again, watermark);

    // A change to one block lists that block alone.
    let referred = owner
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::Publish {
                block: stays,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
        })
        .await;
    assert!(referred.is_ok(), "{referred:?}");
    let (changed, left, watermark_after) = changes(&mut owner, work, watermark, 100).await;
    assert_eq!(changed, vec![stays]);
    assert!(left.is_empty());
    assert!(
        watermark_after > watermark,
        "the sequence moves with a write"
    );

    // A block moved out takes what is below it, and both are named; one moved in is listed.
    let moved = owner.move_to(first, BlockParent::Block(other)).await;
    assert!(moved.is_ok(), "{moved:?}");
    let moved = owner.move_to(elsewhere, BlockParent::Block(work)).await;
    assert!(moved.is_ok(), "{moved:?}");
    let (changed, left, watermark_moved) = changes(&mut owner, work, watermark_after, 1).await;
    assert_eq!(changed, vec![elsewhere]);
    assert_eq!(left, sorted(vec![first, inner]));

    // A block trashed is named as one that left.
    let doomed = owner.create_block(BlockParent::Block(work)).await;
    let (_, _, before_doomed) = changes(&mut owner, work, watermark_moved, 100).await;
    let trashed = owner.move_to(doomed, BlockParent::Detached).await;
    assert!(trashed.is_ok(), "{trashed:?}");
    let (_, left, _) = changes(&mut owner, work, before_doomed, 100).await;
    assert_eq!(left, vec![doomed], "trashed: it left the tree");

    // Several blocks trashed are all named, however the pages are cut, and a tree
    // nothing left or joined is told of none of it.
    let (_, _, before_many) = changes(&mut owner, work, watermark_moved, 100).await;
    let mut many = Vec::new();
    for _ in 0..3 {
        let block = owner.create_block(BlockParent::Block(work)).await;
        let trashed = owner.move_to(block, BlockParent::Detached).await;
        assert!(trashed.is_ok(), "{trashed:?}");
        many.push(block);
    }
    for limit in [1, 2, 100] {
        let (_, left, _) = changes(&mut owner, work, before_many, limit).await;
        for block in &many {
            assert!(
                left.contains(block),
                "limit {limit}: {block} missing in {left:?}"
            );
        }
    }
    let (changed, left, _) = changes(&mut owner, lone, 0, 100).await;
    assert_eq!(changed, vec![lone], "only its own tree");
    assert!(left.is_empty(), "{left:?}");

    // More blocks trashed than a page holds are all named: the removal log pages on the
    // same cursor as the blocks, which a page's limit cuts.
    let (_, _, before_crowd) = changes(&mut owner, work, 0, 100).await;
    let mut crowd = Vec::new();
    for _ in 0..520 {
        let block = owner.create_block(BlockParent::Block(work)).await;
        let trashed = owner.move_to(block, BlockParent::Detached).await;
        assert!(trashed.is_ok(), "{trashed:?}");
        crowd.push(block);
    }
    let (_, left, _) = changes(&mut owner, work, before_crowd, 500).await;
    for block in &crowd {
        assert!(
            left.contains(block),
            "{block} was left out of {} named",
            left.len()
        );
    }

    // A block trashed is named when each page holds one block.
    let late = owner.create_block(BlockParent::Block(work)).await;
    let (_, _, before_late) = changes(&mut owner, work, 0, 100).await;
    let trashed = owner.move_to(late, BlockParent::Detached).await;
    assert!(trashed.is_ok(), "{trashed:?}");
    let page = owner
        .call(|peer| {
            peer.ask::<be_block_protocol::ChangedPage>(
                be_block_protocol::Query::Tree {
                    entry: work,
                    after: ChangeCursor {
                        changed: before_late,
                        block: Uuid::max(),
                    },
                    limit: 1,
                },
                false,
            )
        })
        .await
        .expect("the first page");
    let mut left = page.removed.clone();
    let mut after = page.next;
    while let Some(cursor) = after {
        let page = owner
            .call(|peer| {
                peer.ask::<be_block_protocol::ChangedPage>(
                    be_block_protocol::Query::Tree {
                        entry: work,
                        after: cursor,
                        limit: 1,
                    },
                    false,
                )
            })
            .await
            .expect("a later page");
        left.extend(page.removed);
        after = page.next;
    }
    assert!(
        left.contains(&late),
        "trashed and paged one at a time: {left:?}"
    );

    let still = owner.try_read_block(stays).await;
    assert!(still.is_ok(), "a live block went: {still:?}");

    // A block that left a tree is named to that tree even if it moved on again after: each
    // tree it left hears of it.
    let third = owner.create_block(BlockParent::Root).await;
    let hopper = owner.create_block(BlockParent::Block(work)).await;
    let (_, _, before_hop) = changes(&mut owner, work, 0, 100).await;
    for target in [other, third] {
        let moved = owner.move_to(hopper, BlockParent::Block(target)).await;
        assert!(moved.is_ok(), "{moved:?}");
    }
    let (_, left, _) = changes(&mut owner, work, before_hop, 100).await;
    assert!(
        left.contains(&hopper),
        "work was not told it left: {left:?}"
    );
    let (_, left, _) = changes(&mut owner, other, before_hop, 100).await;
    assert!(
        left.contains(&hopper),
        "other was not told it left: {left:?}"
    );

    // A page that stops on another tree's change names none of its blocks: the cursors only
    // ever hold the caller's own blocks, or none.
    let (_, _, before_busy) = changes(&mut owner, lone, 0, 100).await;
    let mut foreign = Vec::new();
    for _ in 0..20 {
        foreign.push(owner.create_block(BlockParent::Block(other)).await);
    }
    let mut after = ChangeCursor {
        changed: before_busy,
        block: Uuid::max(),
    };
    let mut pages = 0;
    loop {
        let page = owner
            .call(|peer| {
                peer.ask::<be_block_protocol::ChangedPage>(
                    be_block_protocol::Query::Tree {
                        entry: lone,
                        after: after,
                        limit: 100,
                    },
                    false,
                )
            })
            .await
            .expect("a page of a tree of its own");
        pages += 1;
        assert!(pages < 50, "the pages never ended");
        match page.next {
            Some(next) => {
                assert!(
                    !foreign.contains(&next.block),
                    "a cursor named another tree's block {}",
                    next.block
                );
                after = next;
            }
            None => break,
        }
    }

    // A block below the top is asked about as the tree below it: an entry point of a reader who
    // may not view its parent.
    let nested = owner.create_block(BlockParent::Block(work)).await;
    let below = owner.create_block(BlockParent::Block(nested)).await;
    let (changed, _, _) = changes(&mut owner, nested, 0, 100).await;
    assert_eq!(changed, sorted(vec![nested, below]));

    harness.stop().await;
}
