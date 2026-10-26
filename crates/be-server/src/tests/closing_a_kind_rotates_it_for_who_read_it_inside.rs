use super::*;
use be_graph::{ANYONE_INSIDE, Access, SETTINGS_TYPE};

async fn create_under(client: &mut TestClient, content_type: Uuid, parent: Uuid) -> Uuid {
    let block = Uuid::new_v4();
    let key = Uuid::new_v4();
    let response = client
        .send(|request| ClientMessage::CreateBlock {
            request,
            block,
            content_type,
            parent: BlockParent::Block(parent),
            metadata: Vec::new(),
            references: Vec::new(),
            key,
            keys: KeySet {
                keys: vec![row(key, block, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Block { .. }),
        "{response:?}"
    );
    block
}

async fn rotations(client: &mut TestClient) -> Vec<Uuid> {
    let response = client
        .send(|request| ClientMessage::GetKeyWork { request })
        .await;
    let ServerMessage::KeyWork { work, .. } = response else {
        panic!("reading key work failed: {response:?}");
    };
    work.rotations
}

#[tokio::test]
async fn closing_a_kind_rotates_it_for_who_read_it_inside() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    guest.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let note = owner.create_block(BlockParent::Block(workspace)).await;
    let settings = create_under(&mut owner, SETTINGS_TYPE, workspace).await;
    let kind = create_under(&mut owner, Uuid::from_u128(0x7a65), settings).await;
    let granted = owner.grant(note, guest_id, Access::View).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    let opened = owner.grant(kind, ANYONE_INSIDE, Access::View).await;
    assert!(matches!(opened, ServerMessage::Ok { .. }), "{opened:?}");
    assert!(!rotations(&mut owner).await.contains(&kind));

    let closed = owner.grant(kind, ANYONE_INSIDE, Access::None).await;
    assert!(matches!(closed, ServerMessage::Ok { .. }), "{closed:?}");
    assert!(
        rotations(&mut owner).await.contains(&kind),
        "the guest read the kind while it was open, so closing it rotates its key"
    );
    harness.stop().await;
}
