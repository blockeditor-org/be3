use be_block_graph::Access;

use super::*;

/// A Join member (plans/hosted_blocks.md) may connect to whoever hosts the block and nothing
/// more: it finds the block in Shared with me by its type, as one known to exist, and is served
/// none of what a viewer is: no name or head, contents, history, keys or objects.
#[tokio::test]
async fn a_join_member_is_served_nothing_a_viewer_is() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let author = Author::new(owner.id());
    let mut viewer = harness.member("viewer@example.com").await;
    let mut player = harness.member("player@example.com").await;
    let (viewer_id, player_id) = (viewer.id(), player.id());
    for other in [&mut viewer, &mut player] {
        other.personal().await;
        let link = owner.link().await;
        let accepted = other.accept(link, &[]).await;
        assert!(accepted.is_ok(), "{accepted:?}");
    }

    let games = owner.create_block(BlockParent::Root).await;
    let world = owner.create_block(BlockParent::Block(games)).await;
    let (commit, chunks) = author.compose(b"the whole world", 1_000, None);
    owner
        .upload(&author.store, &author.objects_of(commit, &chunks))
        .await;
    let published = owner
        .publish(&author, world, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(
        matches!(
            published,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{published:?}"
    );
    for (account, access) in [(viewer_id, Access::View), (player_id, Access::Join)] {
        let granted = owner.grant(world, account, access).await;
        assert!(granted.is_ok(), "{granted:?}");
    }

    let listed = player
        .call(|peer| {
            peer.read::<be_block_protocol::Listed>(be_block_protocol::Query::List {
                listing: Listing::Shared,
                after: None,
            })
        })
        .await
        .expect("listing failed")
        .blocks;
    assert_eq!(
        listed
            .iter()
            .map(|summary| (summary.id, summary.content_type, summary.access))
            .collect::<Vec<_>>(),
        vec![(world, CONTENT, Access::Join)],
        "a Join member finds the block in Shared with me, by its type"
    );

    let seen = viewer.read_block(world).await;
    assert!(seen.head.is_some() && seen.key.is_some());
    let known = player.read_block(world).await;
    assert_eq!(
        (
            known.access,
            known.head,
            known.key,
            known.metadata.is_empty(),
            known.parent
        ),
        (Access::Join, None, None, true, None),
        "a Join member reads the block as one known to exist"
    );

    assert!(contents(&mut viewer, world).await.is_ok());
    let response = contents(&mut player, world).await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "a Join member read the block's contents: {response:?}"
    );

    assert!(history(&mut viewer, world).await.is_ok());
    let response = history(&mut player, world).await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "a Join member read the block's history: {response:?}"
    );

    let keys = viewer.keys(Some(vec![world])).await;
    assert!(
        !keys.keys.is_empty(),
        "the viewer is served the block's keys"
    );
    let keys = player.keys(Some(vec![world])).await;
    assert!(
        keys.keys.is_empty() && keys.wrapped.is_empty(),
        "a Join member was served keys of the block"
    );

    for chunk in &chunks {
        assert!(viewer.download(*chunk).await.unwrap().is_some());
        assert_eq!(
            player.download(*chunk).await.unwrap(),
            None,
            "a Join member was served an object of the block"
        );
    }

    harness.stop().await;
}

async fn contents(
    client: &mut TestClient,
    block: Uuid,
) -> Result<be_block_protocol::Contents, Error> {
    client
        .call(|peer| {
            peer.read::<be_block_protocol::Contents>(be_block_protocol::Query::Blocks {
                blocks: vec![block],
                contents: true,
                held: Vec::new(),
            })
        })
        .await
}

async fn history(client: &mut TestClient, block: Uuid) -> Result<Vec<HistoryEntry>, Error> {
    client
        .call(|peer| peer.read::<Vec<HistoryEntry>>(be_block_protocol::Query::History(block)))
        .await
}
