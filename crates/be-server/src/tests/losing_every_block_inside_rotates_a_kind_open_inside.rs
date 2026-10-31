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

async fn rotations(client: &mut TestClient) -> Vec<Uuid> {
    let response = client.call(|peer| client::keys::get_key_work(peer)).await;
    let Ok(work) = response else {
        panic!("reading key work failed: {response:?}");
    };
    work.rotations
}

#[tokio::test]
async fn losing_every_block_inside_rotates_a_kind_open_inside() {
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
    let first = owner.create_block(BlockParent::Block(workspace)).await;
    let second = owner.create_block(BlockParent::Block(workspace)).await;
    let settings = create_under(&mut owner, SETTINGS_TYPE, workspace).await;
    let kind = create_under(&mut owner, Uuid::from_u128(0x7a65), settings).await;
    let opened = owner.grant(kind, ANYONE_INSIDE, Access::View).await;
    assert!(matches!(opened, Ok(())), "{opened:?}");
    for note in [first, second] {
        let granted = owner.grant(note, guest_id, Access::View).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }

    let revoked = owner.grant(first, guest_id, Access::None).await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    assert!(
        !rotations(&mut owner).await.contains(&kind),
        "the guest still views a block inside, so still reads the kind"
    );

    let revoked = owner.grant(second, guest_id, Access::None).await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    assert!(
        rotations(&mut owner).await.contains(&kind),
        "the kind's key rotates once the guest views nothing inside"
    );
    harness.stop().await;
}
