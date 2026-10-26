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

async fn move_to(client: &mut TestClient, block: Uuid, parent: Uuid) {
    let response = client
        .send(|request| ClientMessage::SetParent {
            request,
            block,
            parent: BlockParent::Block(parent),
            keys: KeySet::default(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
}

#[tokio::test]
async fn moving_a_block_out_of_what_is_shared_rotates_its_keys() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    guest.accept(link, &[]).await;

    let shared = owner.create_block(BlockParent::Root).await;
    let private = owner.create_block(BlockParent::Root).await;
    let folder = owner.create_block(BlockParent::Block(shared)).await;
    let note = owner.create_block(BlockParent::Block(shared)).await;
    let granted = owner.grant(shared, guest_id, Access::View).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");

    move_to(&mut owner, note, folder).await;
    assert!(
        rotations(&mut owner).await.is_empty(),
        "the guest still views the note where it went"
    );

    move_to(&mut owner, note, private).await;
    assert!(
        rotations(&mut owner).await.contains(&note),
        "the guest no longer views the note, so its keys rotate"
    );

    let workspace = owner.create_block(BlockParent::Root).await;
    let inside = owner.create_block(BlockParent::Block(workspace)).await;
    let settings = create_under(&mut owner, SETTINGS_TYPE, workspace).await;
    let kind = create_under(&mut owner, Uuid::from_u128(0x7a65), settings).await;
    let opened = owner.grant(kind, ANYONE_INSIDE, Access::View).await;
    assert!(matches!(opened, ServerMessage::Ok { .. }), "{opened:?}");
    let granted = owner.grant(inside, guest_id, Access::View).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    assert!(!rotations(&mut owner).await.contains(&kind));

    move_to(&mut owner, inside, private).await;
    let rotating = rotations(&mut owner).await;
    assert!(
        rotating.contains(&kind),
        "the guest views nothing inside the workspace now, so its open kind rotates: {rotating:?}"
    );
    assert!(
        !rotating.contains(&inside),
        "the guest keeps the note its own grant shares"
    );
    harness.stop().await;
}
