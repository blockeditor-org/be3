use be_block_graph::{Access, SETTINGS_TYPE};

use super::*;

async fn summaries(client: &mut TestClient, blocks: Vec<Uuid>) -> Vec<Option<Uuid>> {
    client
        .call(|peer| client::blocks::read_summaries(peer, blocks))
        .await
        .expect("read-summaries failed")
        .into_iter()
        .map(|summary| summary.map(|summary| summary.id))
        .collect()
}

#[tokio::test]
async fn read_summaries_leaves_out_what_the_caller_may_not_know_of() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let mut viewer = harness.client().await;
    let viewer_id = viewer.register("viewer@example.com").await;
    viewer.personal().await;
    let link = owner.link(None).await;
    let _ = viewer.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(workspace, viewer_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    // The owner keeps a block only they may read in their settings in the workspace.
    let settings = Uuid::new_v4();
    let settings_key = Uuid::new_v4();
    owner
        .create_typed(
            settings,
            SETTINGS_TYPE,
            BlockParent::Block(workspace),
            settings_key,
            KeySet {
                keys: vec![row(settings_key, settings, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        )
        .await;
    let secret = Uuid::new_v4();
    let key = Uuid::new_v4();
    let made = owner
        .call(|peer| {
            client::blocks::create_block(
                peer,
                secret,
                CONTENT,
                BlockParent::Block(settings),
                Vec::new(),
                Vec::new(),
                key,
                KeySet {
                    keys: vec![row(key, secret, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
                true,
            )
        })
        .await;
    assert!(made.is_ok(), "{made:?}");
    let missing = Uuid::new_v4();

    assert_eq!(
        summaries(&mut viewer, vec![workspace, missing]).await,
        vec![Some(workspace), None],
        "a block that does not exist"
    );
    // Another member's owner-only block is known to exist, as its folder lists it, and no more.
    let read = viewer
        .call(|peer| client::blocks::read_summaries(peer, vec![secret]))
        .await
        .expect("read-summaries failed");
    let [Some(known)] = read.as_slice() else {
        panic!("{read:?}");
    };
    assert!(
        !known.access.can_view()
            && known.metadata.is_empty()
            && known.parent.is_none()
            && known.head.is_none(),
        "{known:?}"
    );
    assert_eq!(
        summaries(&mut owner, vec![secret]).await,
        vec![Some(secret)]
    );

    // A block's agent knows nothing outside its block.
    let terminal = owner.create_block(BlockParent::Root).await;
    let (_, token) = owner.block_agent(organization, terminal).await.unwrap();
    let mut agent = harness.second_connection(&token, organization).await;
    assert_eq!(
        summaries(&mut agent, vec![terminal, workspace]).await,
        vec![Some(terminal), None]
    );

    // A call naming more blocks than one answer may is refused.
    let asked = vec![workspace; be_block_protocol::MAX_SUMMARIES_ASKED + 1];
    let response = viewer
        .call(|peer| client::blocks::read_summaries(peer, asked))
        .await;
    assert!(
        refused_as(&response, ErrorCode::InvalidRequest),
        "{response:?}"
    );

    harness.stop().await;
}
