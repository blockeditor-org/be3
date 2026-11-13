use super::a_block_in_unfiled_stays_live_without_a_listing_until_it_is_deleted::make;
use super::*;

#[tokio::test]
async fn nobody_makes_or_writes_to_another_accounts_unfiled() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let unfiled = be_block::unfiled(organization, account);

    let mut other = harness.client().await;
    let intruder = other.register("other@example.com").await;
    other.personal().await;
    let taken = make(
        &mut other,
        unfiled,
        be_block::UNFILED_TYPE,
        BlockParent::Root,
    )
    .await;
    assert!(
        refused_as(&taken, ErrorCode::PermissionDenied),
        "the id is kept for its account before that account makes it: {taken:?}"
    );
    let taken = make(&mut other, unfiled, CONTENT, BlockParent::Root).await;
    assert!(refused_as(&taken, ErrorCode::PermissionDenied), "{taken:?}");
    let elsewhere = make(
        &mut other,
        Uuid::new_v4(),
        be_block::UNFILED_TYPE,
        BlockParent::Root,
    )
    .await;
    assert!(
        refused_as(&elsewhere, ErrorCode::PermissionDenied),
        "an Unfiled is made only at its account's id: {elsewhere:?}"
    );
    let own = be_block::unfiled(organization, intruder);
    let own_folder = make(&mut other, own, CONTENT, BlockParent::Root).await;
    assert!(
        refused_as(&own_folder, ErrorCode::PermissionDenied),
        "even its own account's id takes only an Unfiled: {own_folder:?}"
    );

    let made = make(
        &mut owner,
        unfiled,
        be_block::UNFILED_TYPE,
        BlockParent::Root,
    )
    .await;
    assert!(made.is_ok(), "{made:?}");
    let filed = owner.create_block(BlockParent::Block(unfiled)).await;

    let inside = make(
        &mut other,
        Uuid::new_v4(),
        CONTENT,
        BlockParent::Block(unfiled),
    )
    .await;
    assert!(
        refused_as(&inside, ErrorCode::BlockNotFound),
        "another account cannot make a block in it: {inside:?}"
    );
    let mine = other.create_block(BlockParent::Root).await;
    let moved = other.move_to(mine, BlockParent::Block(unfiled)).await;
    assert!(
        refused_as(&moved, ErrorCode::BlockNotFound),
        "or move one into it: {moved:?}"
    );
    let read = other
        .try_read(filed).await;
    assert!(
        refused_as(&read, ErrorCode::BlockNotFound),
        "or read what it holds: {read:?}"
    );
    let taken_out = other.move_to(filed, BlockParent::Detached).await;
    assert!(
        refused_as(&taken_out, ErrorCode::BlockNotFound),
        "or take anything out of it: {taken_out:?}"
    );
    assert!(owner.read_block(filed).await.held);

    harness.stop().await;
}
