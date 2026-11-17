use super::*;

/// A rotation that leaves a block below its root on its old key is refused, and with it every
/// op of its write: what an op before it stored is rolled back too.
#[tokio::test]
async fn a_rotation_missing_one_blocks_key_is_refused_whole() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let shared = owner.scope_root(owner_id, BlockParent::Root).await;
    let (inside, _) = owner.inside(&shared).await;

    let (scope, top, below, extra) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
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
    let renamed = || Op::SetMetadata {
        block: shared.block,
        metadata: b"renamed".to_vec(),
        references: Vec::new(),
    };
    let stored = Op::PutKeys(be_block_protocol::PutKeys::Rows(KeySet {
        keys: vec![row(extra, shared.block, KeyKind::Data)],
        wrapped: vec![wrap(extra, WrappedBy::Account(owner_id), b"another key")],
    }));
    let rotate = |keys| {
        Op::PutKeys(be_block_protocol::PutKeys::Rotate {
            roots: vec![shared.block],
            keys,
        })
    };

    let written = client::server::write(
        &owner.peer,
        Some(organization),
        vec![renamed(), stored, rotate(partial.clone())],
    )
    .await
    .expect("the connection stayed open");
    let Err(WriteError { index, error }) = written else {
        panic!("a rotation that left a block on its old key was taken: {written:?}");
    };
    assert_eq!(
        (index, error.code),
        (2, ErrorCode::InvalidRequest),
        "{error:?}"
    );
    assert!(
        owner.read_block(shared.block).await.metadata.is_empty(),
        "the metadata set before the rotation was kept"
    );
    let keys = owner.keys(Some(vec![shared.block, inside])).await;
    assert!(
        keys.keys
            .iter()
            .all(|key| ![scope, top, extra].contains(&key.id)),
        "keys of a refused write were stored: {keys:?}"
    );

    let mut whole = partial;
    whole.keys.push(row(below, inside, KeyKind::Data));
    whole.wrapped.push(wrap(
        below,
        WrappedBy::Key(scope),
        b"the new data key below",
    ));
    let written = owner
        .call(|peer| peer.writes(vec![renamed(), rotate(whole)]))
        .await;
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(owner.read_block(inside).await.key, Some(below));
    assert_eq!(owner.read_block(shared.block).await.metadata, b"renamed");

    harness.stop().await;
}
