use super::*;

#[tokio::test]
async fn moving_a_block_drops_the_wraps_of_its_old_scope() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    owner.organization("moves").await;
    let from = owner.scope_root(account, BlockParent::Root).await;
    let to = owner.scope_root(account, BlockParent::Root).await;
    let (document, old_key) = owner.inside(&from).await;
    let (elsewhere, _) = owner.inside(&to).await;
    assert_eq!(owner.read_block(document).await.scope, Some(from.block));

    let stray = owner
        .call(|peer| {
            client::blocks::set_parent(
                peer,
                document,
                BlockParent::Block(to.block),
                KeySet {
                    keys: vec![row(Uuid::new_v4(), elsewhere, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
            )
        })
        .await;
    assert!(
        matches!(stray, Err(_)),
        "a move stored a key for a block it did not move: {stray:?}"
    );
    assert_eq!(
        owner.read_block(document).await.parent,
        Some(BlockParent::Block(from.block))
    );

    let fresh = Uuid::new_v4();
    let response = owner
        .call(|peer| {
            client::blocks::set_parent(
                peer,
                document,
                BlockParent::Block(to.block),
                KeySet {
                    keys: vec![KeyRow {
                        id: fresh,
                        block: document,
                        kind: KeyKind::Data,
                        epoch: 2,
                    }],
                    wrapped: vec![
                        wrap(
                            old_key,
                            WrappedBy::Key(to.scope),
                            b"old data under the new scope",
                        ),
                        wrap(
                            fresh,
                            WrappedBy::Key(to.scope),
                            b"fresh data under the new scope",
                        ),
                    ],
                },
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let moved = owner.read_block(document).await;
    assert_eq!(moved.key, Some(fresh));
    assert_eq!(moved.scope, Some(to.block));

    let keys = owner.keys(None).await;
    let mut wraps: Vec<_> = keys
        .wrapped
        .iter()
        .filter(|wrap| wrap.key == old_key || wrap.key == fresh)
        .map(|wrap| (wrap.key, wrap.wrapped_by))
        .collect();
    wraps.sort_unstable();
    let mut expected = vec![
        (old_key, WrappedBy::Key(to.scope)),
        (fresh, WrappedBy::Key(to.scope)),
    ];
    expected.sort_unstable();
    assert_eq!(
        wraps, expected,
        "the moved block's keys are still wrapped under the scope it left"
    );

    harness.stop().await;
}
