use super::*;
use be_graph::{ANYONE_INSIDE, Access, SETTINGS_TYPE};

async fn create_under(client: &mut TestClient, content_type: Uuid, parent: Uuid) -> Uuid {
    let block = Uuid::new_v4();
    let key = Uuid::new_v4();
    let response = client
        .call(|peer| {
            client::blocks::create_block(
                peer,
                block,
                content_type,
                BlockParent::Block(parent),
                Vec::new(),
                Vec::new(),
                key,
                KeySet {
                    keys: vec![row(key, block, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
            )
        })
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
    block
}

async fn access_of(client: &mut TestClient, block: Uuid) -> Access {
    match client
        .call(|peer| client::blocks::read_block(peer, block))
        .await
    {
        Ok(block) => block.access,
        _ => Access::None,
    }
}

#[tokio::test]
async fn a_settings_kind_opens_to_anyone_who_views_a_block_inside() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let folder = owner.create_block(BlockParent::Block(workspace)).await;
    let note = owner.create_block(BlockParent::Block(folder)).await;
    let settings = create_under(&mut owner, SETTINGS_TYPE, workspace).await;
    let kind = create_under(&mut owner, Uuid::from_u128(0x7a65), settings).await;
    let elsewhere = owner.create_block(BlockParent::Root).await;

    let granted = owner.grant(note, guest_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    assert!(!access_of(&mut guest, kind).await.can_view());

    let opened = owner.grant(kind, ANYONE_INSIDE, Access::View).await;
    assert!(matches!(opened, Ok(())), "{opened:?}");
    assert_eq!(access_of(&mut guest, kind).await, Access::View);
    assert!(!access_of(&mut guest, settings).await.can_view());
    let listed = guest
        .call(|peer| client::blocks::list_blocks(peer, Listing::OpenSettings(note)))
        .await;
    let Ok(blocks) = listed else {
        panic!("listing the open settings failed: {listed:?}");
    };
    assert_eq!(
        blocks
            .iter()
            .map(|summary| (summary.id, summary.opens_inside))
            .collect::<Vec<_>>(),
        [(kind, true)]
    );

    let key = Uuid::new_v4();
    let stored = owner
        .call(|peer| {
            client::keys::put_keys(
                peer,
                KeySet {
                    keys: vec![row(key, kind, KeyKind::Scope)],
                    wrapped: Vec::new(),
                },
            )
        })
        .await;
    assert!(matches!(stored, Ok(())), "{stored:?}");
    let response = owner.call(|peer| client::keys::get_key_work(peer)).await;
    let Ok(work) = response else {
        panic!("reading key work failed: {response:?}");
    };
    assert!(
        work.unsealed
            .iter()
            .any(|unsealed| unsealed.key == key && unsealed.account == guest_id),
        "{work:?}"
    );

    for (block, access) in [
        (elsewhere, Access::View),
        (kind, Access::Edit),
        (settings, Access::View),
    ] {
        let refused = owner.grant(block, ANYONE_INSIDE, access).await;
        assert!(
            refused_as(&refused, ErrorCode::InvalidRequest),
            "{refused:?}"
        );
    }
    let closed = owner.grant(kind, ANYONE_INSIDE, Access::None).await;
    assert!(matches!(closed, Ok(())), "{closed:?}");
    assert!(!access_of(&mut guest, kind).await.can_view());

    harness.stop().await;
}
