use super::*;

/// A block another account offers this one to own is listed (`Query::OwnerOffers`) as soon as
/// the server tells of it, with no read asked for, and leaves the list once the offer is
/// withdrawn or this account accepts it.
#[test]
fn a_block_offered_to_this_account_is_listed_until_the_offer_is_withdrawn_or_accepted() {
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
        block
    });
    ask_for(vec![Query::Shared, Query::OwnerOffers]);
    wait_until("listed the shared block", |held| {
        held.graph.get(block).is_some() && held.graph.ready(Query::OwnerOffers)
    });
    let offered = |held: &Shared| {
        held.graph
            .query(Query::OwnerOffers)
            .iter()
            .any(|node| node.id == block)
    };
    assert!(
        !crate::wait_until("read the offers", |held| Some(offered(held))),
        "a block nobody offered is listed as offered"
    );

    runtime
        .block_on(peer.transfer(block, harness.account))
        .expect("the owner offers the block to this account");
    wait_until("listed the block offered", offered);

    runtime
        .block_on(peer.transfer(block, owner.session.account))
        .expect("the owner withdraws the offer");
    wait_until("dropped the offer withdrawn", |held| !offered(held));

    runtime
        .block_on(peer.transfer(block, harness.account))
        .expect("the owner offers the block again");
    wait_until("listed the block offered again", offered);
    let (sender, receiver) = std::sync::mpsc::channel();
    accept_ownership(block, move |accepted| {
        let _ = sender.send(accepted);
    });
    receiver
        .recv()
        .expect("the worker answered")
        .expect("this account accepts the block");
    wait_until("dropped the offer accepted", |held| !offered(held));
}
