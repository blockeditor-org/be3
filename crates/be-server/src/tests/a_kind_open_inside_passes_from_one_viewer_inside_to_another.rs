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

async fn unsealed(client: &mut TestClient) -> Vec<(Uuid, Uuid)> {
    let response = client
        .send(|request| ClientMessage::GetKeyWork { request })
        .await;
    let ServerMessage::KeyWork { work, .. } = response else {
        panic!("reading key work failed: {response:?}");
    };
    work.unsealed
        .iter()
        .map(|unsealed| (unsealed.key, unsealed.account))
        .collect()
}

async fn put(client: &mut TestClient, wrapped: Vec<WrappedKey>) -> ServerMessage {
    client
        .send(|request| ClientMessage::PutKeys {
            request,
            keys: KeySet {
                keys: Vec::new(),
                wrapped,
            },
        })
        .await
}

#[tokio::test]
async fn a_kind_open_inside_passes_from_one_viewer_inside_to_another() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut sharer = harness.client().await;
    let sharer_id = sharer.register("sharer@example.com").await;
    sharer.personal().await;
    let mut viewer = harness.client().await;
    let viewer_id = viewer.register("viewer@example.com").await;
    viewer.personal().await;
    let link = owner.link(None).await;
    sharer.accept(link, &[]).await;
    let link = sharer.link(None).await;
    viewer.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let folder = owner.create_block(BlockParent::Block(workspace)).await;
    let note = owner.create_block(BlockParent::Block(folder)).await;
    let settings = create_under(&mut owner, SETTINGS_TYPE, workspace).await;
    let kind = create_under(&mut owner, Uuid::from_u128(0x7a65), settings).await;
    let granted = owner.grant(folder, sharer_id, Access::Manage).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    let granted = sharer.grant(note, viewer_id, Access::View).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    let opened = owner.grant(kind, ANYONE_INSIDE, Access::View).await;
    assert!(matches!(opened, ServerMessage::Ok { .. }), "{opened:?}");

    let key = Uuid::new_v4();
    let stored = owner
        .send(|request| ClientMessage::PutKeys {
            request,
            keys: KeySet {
                keys: vec![row(key, kind, KeyKind::Scope)],
                wrapped: vec![wrap(key, WrappedBy::Account(sharer_id), b"for the sharer")],
            },
        })
        .await;
    assert!(matches!(stored, ServerMessage::Ok { .. }), "{stored:?}");

    assert!(
        unsealed(&mut sharer).await.contains(&(key, viewer_id)),
        "the viewer is nobody's contact but the sharer's, and the sharer is not asked to pass the kind on"
    );
    let passed = put(
        &mut sharer,
        vec![wrap(key, WrappedBy::Account(viewer_id), b"for the viewer")],
    )
    .await;
    assert!(matches!(passed, ServerMessage::Ok { .. }), "{passed:?}");
    assert!(!unsealed(&mut sharer).await.contains(&(key, viewer_id)));
    assert!(!unsealed(&mut owner).await.contains(&(key, viewer_id)));

    let other = Uuid::new_v4();
    let stored = owner
        .send(|request| ClientMessage::PutKeys {
            request,
            keys: KeySet {
                keys: vec![row(other, note, KeyKind::Scope)],
                wrapped: vec![wrap(other, WrappedBy::Account(sharer_id), b"for the sharer")],
            },
        })
        .await;
    assert!(matches!(stored, ServerMessage::Ok { .. }), "{stored:?}");
    let refused = put(
        &mut viewer,
        vec![wrap(other, WrappedBy::Account(sharer_id), b"from a viewer")],
    )
    .await;
    assert!(
        refused_as(&refused, ErrorCode::PermissionDenied),
        "a viewer passed on a key that is not a kind open inside: {refused:?}"
    );

    harness.stop().await;
}
