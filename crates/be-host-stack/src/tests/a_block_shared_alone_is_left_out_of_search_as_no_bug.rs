use super::*;

/// A block shared alone with this account, below a parent it may not view, lies in no root this
/// account may read. Search asks the mirror for the parent to place it, the mirror lists none,
/// and the block is left out of search: as the server's answer says this account's view starts
/// there (`entry`), not as a block the mirror cannot place, which is counted and logged as a fault.
#[test]
fn a_block_shared_alone_is_left_out_of_search_as_no_bug() {
    let harness = Harness::start();
    let sharer = Someone::register(
        harness.server.directory(),
        &harness.url,
        "sharer@example.com",
        "Sharer",
    );
    let personal = harness.connect_personal();
    let code = made_link(None);
    let runtime = runtime();
    let peer = runtime.block_on(sharer.peer(&harness.url, personal));
    runtime
        .block_on(be_block_client::contacts::accept_link(
            &peer.connection(),
            &sharer.phrase.secret(),
            &sharer.me().card(),
            &code,
            be_block_client::contacts::Expected::Contact,
            &Default::default(),
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .verified_identity(harness.account)
        .expect("the sharer verified this account through the link");
    let chat = runtime.block_on(async {
        let workspace = peer
            .create::<be_type_workspace::WorkspaceContent>(be_block_graph::BlockParent::Root)
            .await
            .unwrap();
        let chat = peer
            .create::<be_type_chat::ChatContent>(be_block_graph::BlockParent::Block(workspace))
            .await
            .unwrap();
        peer.share(chat, harness.account, be_block_graph::Access::Edit, &public)
            .await
            .unwrap();
        chat
    });

    ask_for(vec![Query::Shared]);
    wait_until(
        "listed the shared chat where this account's view starts",
        |held| {
            held.graph
                .get(chat)
                .is_some_and(|node| node.access.can_edit() && node.entry == Some(chat))
        },
    );
    // Queued as an edit here queues it.
    with_shared_mut(|shared| shared.search.reread([chat]));
    retry_now();
    crate::wait_until("indexed everything", |shared| {
        shared.search.indexed().then_some(())
    });
    assert_eq!(
        crate::with_shared(|shared| shared.search.unplaceable),
        Some(0),
        "the shared chat waited to be placed"
    );
}
