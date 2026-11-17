use super::*;

/// A move into another scope wraps every key it takes there under the new scope's key, in the
/// same write: one block below the moved one left out refuses the whole write, the move and the
/// wraps beside it, and with it the move goes.
#[tokio::test]
async fn a_move_missing_one_blocks_wrap_under_its_new_scope_is_refused_whole() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let from = owner.scope_root(owner_id, BlockParent::Root).await;
    let to = owner.scope_root(owner_id, BlockParent::Root).await;
    let (folder, folder_key) = owner.inside(&from).await;
    let (below, below_key) = (Uuid::new_v4(), Uuid::new_v4());
    owner
        .create_keyed(
            below,
            BlockParent::Block(folder),
            below_key,
            KeySet {
                keys: vec![row(below_key, below, KeyKind::Data)],
                wrapped: vec![wrap(
                    below_key,
                    WrappedBy::Key(from.scope),
                    b"below, under the old scope",
                )],
            },
        )
        .await;
    let under_new = |key| wrap(key, WrappedBy::Key(to.scope), b"under the new scope");

    let partial = KeySet {
        keys: Vec::new(),
        wrapped: vec![under_new(folder_key)],
    };
    let moved = owner
        .move_keyed(folder, BlockParent::Block(to.block), partial)
        .await;
    assert!(
        matches!(
            moved,
            Err(Error {
                code: ErrorCode::KeyMissing,
                ..
            })
        ),
        "a move leaving a block below it unreadable in its new scope was taken: {moved:?}"
    );
    assert_eq!(
        owner.read_block(folder).await.parent,
        Some(BlockParent::Block(from.block)),
        "the refused move was kept"
    );
    let keys = owner.keys(Some(vec![folder])).await;
    assert!(
        keys.wrapped
            .iter()
            .all(|wrapped| wrapped.wrapped_by != WrappedBy::Key(to.scope)),
        "a wrap of a refused move was kept: {keys:?}"
    );

    let whole = KeySet {
        keys: Vec::new(),
        wrapped: vec![under_new(folder_key), under_new(below_key)],
    };
    let moved = owner
        .move_keyed(folder, BlockParent::Block(to.block), whole)
        .await;
    assert!(moved.is_ok(), "{moved:?}");
    assert_eq!(
        owner.read_block(folder).await.parent,
        Some(BlockParent::Block(to.block))
    );

    // Within one scope it takes no key anywhere new, so it carries none.
    let (sibling, _) = owner.inside(&to).await;
    let moved = owner.move_to(below, BlockParent::Block(sibling)).await;
    assert!(moved.is_ok(), "{moved:?}");

    harness.stop().await;
}
