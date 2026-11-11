use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn detaching_a_block_rotates_the_keys_of_who_no_longer_views_it() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;

    let shared = owner.create_block(BlockParent::Root).await;
    let note = owner.create_block(BlockParent::Block(shared)).await;
    let granted = owner.grant(shared, guest_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    let work = owner.call(|peer| client::keys::get_key_work(peer)).await;
    assert!(
        work.is_ok_and(|work| work.rotations.is_empty()),
        "nothing rotates while the guest views the note"
    );

    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, note, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let work = owner.call(|peer| client::keys::get_key_work(peer)).await;
    let Ok(work) = work else {
        panic!("reading key work failed: {work:?}");
    };
    assert!(
        work.rotations.contains(&note),
        "the guest no longer views the trashed note, so its keys rotate"
    );
    harness.stop().await;
}
