use super::*;

/// The account's mutes, the offers to it and its notices, each read whole and stored, go
/// through their numbered reads (`worker::Numbered`, `Shared::reads`), so an older answer never
/// overwrites a newer: each is read as the connection comes up, and an offer made reads the
/// offers again.
#[test]
fn the_mutes_offers_and_notices_are_read_through_their_numbered_reads() {
    let harness = Harness::start();
    let owner = Someone::register(
        harness.server.directory(),
        &harness.url,
        "owner@example.com",
        "Owner",
    );
    let personal = harness.connect_personal();
    let code = made_link(None);
    let runtime = runtime();
    let peer = runtime.block_on(owner.peer(&harness.url, personal));
    runtime
        .block_on(be_block_client::contacts::accept_link(
            &peer.connection(),
            &owner.phrase.secret(),
            &owner.me().card(),
            &code,
            be_block_client::contacts::Expected::Contact,
            &Default::default(),
        ))
        .expect("the owner accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the owner lists its contacts");
    let public = peer
        .verified_identity(harness.account)
        .expect("the owner verified this account through the link");
    ask_for(vec![Query::OwnerOffers]);
    wait_until("read the mutes, offers and notices", |held| {
        held.reads.mutes.stored > 0
            && held.reads.owner_offers.stored > 0
            && held.reads.notices.stored > 0
    });
    let before = crate::wait_until("counted the offers read", |held| {
        Some(held.reads.owner_offers.stored)
    });

    let block = runtime.block_on(async {
        let block = peer
            .create::<CounterContent>(be_block_graph::BlockParent::Root)
            .await
            .unwrap();
        peer.share(
            block,
            harness.account,
            be_block_graph::Access::Edit,
            &public,
        )
        .await
        .unwrap();
        peer.transfer(block, harness.account).await.unwrap();
        block
    });
    wait_until("read the offers again and listed the one made", |held| {
        held.reads.owner_offers.stored > before
            && held
                .graph
                .query(Query::OwnerOffers)
                .iter()
                .any(|node| node.id == block)
    });
}
