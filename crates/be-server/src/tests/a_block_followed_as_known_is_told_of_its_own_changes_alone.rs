use super::*;
use be_block_graph::Access;

/// Sets `block`'s metadata and references, with no commit.
async fn describe(client: &mut TestClient, block: Uuid, references: &[Uuid], metadata: &[u8]) {
    let response = client
        .call(|peer| {
            peer.write::<BlockSummary>(be_block_protocol::Op::Publish {
                block,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], references),
                metadata: metadata.to_vec(),
                journal: None,
            })
        })
        .await;
    assert!(response.is_ok(), "{response:?}");
}

/// Reads `block`'s summary and follows it as a block known of (`topic.known`).
async fn follow_known(client: &mut TestClient, block: Uuid) -> Option<BlockSummary> {
    client
        .call(|peer| {
            peer.ask::<Vec<Option<BlockSummary>>>(
                Query::Blocks {
                    blocks: vec![block],
                    contents: false,
                    held: Vec::new(),
                },
                true,
            )
        })
        .await
        .expect("the read is answered")
        .into_iter()
        .next()
        .flatten()
}

/// A block a viewer knows of only through a reference, followed by itself, is told of changes
/// to its own summary alone: once until it is read again, never of a change to a block below
/// it the viewer may not know of, and never once a revoke means the viewer may not know of it,
/// whose roots the revoke tells instead.
#[tokio::test]
async fn a_block_followed_as_known_is_told_of_its_own_changes_alone() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let page = owner.create_block(BlockParent::Root).await;
    let hidden = owner.create_block(BlockParent::Root).await;
    let below = owner.create_block(BlockParent::Block(hidden)).await;
    let (mut viewer, account) =
        super::a_graph_change_reaches_only_the_members_who_may_see_it::join(
            &harness,
            &mut owner,
            organization,
            "viewer@example.com",
            1,
        )
        .await;
    let granted = owner.grant(page, account, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    describe(&mut owner, page, &[hidden], b"page").await;
    let known = Topic::Known(hidden);

    let summary = follow_known(&mut viewer, hidden)
        .await
        .expect("the viewer knows the block exists");
    assert_eq!(summary.access, Access::KnowExists);
    assert_eq!(
        follow_known(&mut viewer, below).await,
        None,
        "nothing lets the viewer know of the block below"
    );

    describe(&mut owner, below, &[], b"a hidden change").await;
    assert!(
        !viewer.told(known.clone()).await,
        "a change below the block, which the viewer may not know of, is not told"
    );

    describe(&mut owner, hidden, &[], b"renamed").await;
    describe(&mut owner, hidden, &[], b"renamed again").await;
    assert!(
        viewer.told(known.clone()).await,
        "a change of its own is told"
    );
    describe(&mut owner, hidden, &[], b"and again").await;
    assert!(
        !viewer.told(known.clone()).await,
        "told once until the summary is read again"
    );
    viewer.read_block(hidden).await;
    describe(&mut owner, hidden, &[], b"once more").await;
    assert!(
        viewer.told(known.clone()).await,
        "a read starts what is told again"
    );
    viewer.read_block(hidden).await;

    // The viewer's grant on the page that references it is revoked: its roots are told, and the
    // follow goes without a word, which would tell it when a block it may not know of changed.
    viewer
        .notifications
        .retain(|notice| !matches!(notice, Notice::Roots));
    let revoked = owner.grant(page, account, Access::None).await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    describe(&mut owner, hidden, &[], b"out of reach").await;
    describe(&mut owner, hidden, &[], b"unseen").await;
    assert!(
        !viewer.told(known).await,
        "nothing is told of a block the viewer may no longer know of"
    );
    assert!(
        viewer
            .notifications
            .iter()
            .any(|notice| matches!(notice, Notice::Roots)),
        "the revoke tells the viewer's roots"
    );
}
