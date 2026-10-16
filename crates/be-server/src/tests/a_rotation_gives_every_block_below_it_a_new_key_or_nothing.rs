use super::*;
use be_graph::Access;

#[tokio::test]
async fn a_rotation_gives_every_block_below_it_a_new_key_or_nothing() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    owner.personal().await;
    let mut reader = harness.client().await;
    let reader_id = reader.register("reader@example.com").await;
    reader.personal().await;
    let shared = owner.scope_root(owner_id, BlockParent::Root).await;
    let (inside, _) = owner.inside(&shared).await;
    let link = owner.link(None).await;
    reader.accept(link, &[]).await;
    let granted = owner.grant(shared.block, reader_id, Access::View).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    reader.join_session(inside).await;

    let revoked = owner.grant(shared.block, reader_id, Access::None).await;
    assert!(matches!(revoked, ServerMessage::Ok { .. }), "{revoked:?}");
    assert_eq!(work(&mut owner).await.rotations, [shared.block]);
    let spoken = reader
        .send(|request| ClientMessage::Relay {
            request,
            block: inside,
            to: None,
            payload: b"still here".to_vec(),
        })
        .await;
    assert!(
        matches!(spoken, ServerMessage::Failed { .. }),
        "a reader taken off a block kept speaking in its session: {spoken:?}"
    );

    let (scope, top, below) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let partial = KeySet {
        keys: vec![
            row(scope, shared.block, KeyKind::Scope),
            row(top, shared.block, KeyKind::Data),
        ],
        wrapped: vec![
            wrap(scope, WrappedBy::Account(owner_id), b"the new scope key"),
            wrap(top, WrappedBy::Key(scope), b"the new data key"),
        ],
    };
    let refused = rotate(&mut owner, shared.block, partial.clone()).await;
    assert!(
        refused_as(&refused, ErrorCode::InvalidRequest),
        "a rotation that left a block on its old key was taken: {refused:?}"
    );
    assert_eq!(work(&mut owner).await.rotations, [shared.block]);
    assert!(
        owner
            .keys(Some(vec![shared.block]))
            .await
            .keys
            .iter()
            .all(|key| key.id != scope),
        "half a rotation was stored"
    );

    let mut whole = partial;
    whole.keys.push(row(below, inside, KeyKind::Data));
    whole.wrapped.push(wrap(
        below,
        WrappedBy::Key(scope),
        b"the new data key below",
    ));
    let rotated = rotate(&mut owner, shared.block, whole).await;
    assert!(matches!(rotated, ServerMessage::Ok { .. }), "{rotated:?}");
    assert!(work(&mut owner).await.rotations.is_empty());
    assert_eq!(owner.read_block(inside).await.key, Some(below));

    harness.stop().await;
}

async fn work(client: &mut TestClient) -> be_protocol::KeyWork {
    let response = client
        .send(|request| ClientMessage::GetKeyWork { request })
        .await;
    let ServerMessage::KeyWork { work, .. } = response else {
        panic!("reading key work failed: {response:?}");
    };
    work
}

async fn rotate(client: &mut TestClient, root: Uuid, keys: KeySet) -> ServerMessage {
    client
        .send(|request| ClientMessage::Rotate {
            request,
            roots: vec![root],
            keys,
        })
        .await
}
