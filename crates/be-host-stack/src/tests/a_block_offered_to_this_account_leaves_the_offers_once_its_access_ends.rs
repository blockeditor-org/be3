use super::*;

/// An offer of ownership ends with the offeree's access: the owner's grant taking this account
/// below View ends the offer in the block's log (`Checked::offered`), and the server tells the
/// roots changed to whoever could view the block before (`regranted`, `After::Revoked`), whose
/// host reads the offers again: the block leaves `Query::OwnerOffers` with no read asked for,
/// and comes back with no offer when the grant does, as the log's offer stays ended.
#[test]
fn a_block_offered_to_this_account_leaves_the_offers_once_its_access_ends() {
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
    ask_for(vec![Query::Shared, Query::OwnerOffers]);
    let offered = |held: &Shared| {
        held.graph
            .query(Query::OwnerOffers)
            .iter()
            .any(|node| node.id == block)
    };
    wait_until("listed the block offered", offered);

    runtime
        .block_on(peer.grant(block, harness.account, be_block_graph::Access::None))
        .expect("the owner takes this account's grant away");
    wait_until("dropped the offer whose access ended", |held| {
        !offered(held) && held.graph.ready(Query::OwnerOffers)
    });

    // Shared again, then another block offered after it: the offers read that lists that one
    // was asked after the grant came back.
    let other = runtime.block_on(async {
        peer.share(
            block,
            harness.account,
            be_block_graph::Access::Edit,
            &public,
        )
        .await
        .unwrap();
        let other = peer
            .create::<CounterContent>(be_block_graph::BlockParent::Root)
            .await
            .unwrap();
        peer.share(
            other,
            harness.account,
            be_block_graph::Access::Edit,
            &public,
        )
        .await
        .unwrap();
        peer.transfer(other, harness.account).await.unwrap();
        other
    });
    wait_until("listed the other block offered", |held| {
        held.graph
            .query(Query::OwnerOffers)
            .iter()
            .any(|node| node.id == other)
    });
    assert!(
        !crate::wait_until("read the offers again", |held| Some(offered(held))),
        "an offer that ended with this account's access came back with the grant"
    );
}
