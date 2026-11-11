use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_member_who_loses_view_when_a_block_moves_somewhere_hidden_stops_watching_it() {
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
    let hidden = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(shared, guest_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    guest.watch(note).await;
    assert!(
        guest.heard.iter().any(|heard| matches!(heard, Heard::Watch(..))),
        "the guest watches the note it may view"
    );

    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, note, BlockParent::Block(hidden), KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    guest.round_trip().await;
    guest.round_trip().await;
    assert!(
        !guest.heard.iter().any(|heard| matches!(heard, Heard::Watch(..))),
        "the guest cannot view the note in the hidden folder, so its watch ended"
    );
    harness.stop().await;
}
