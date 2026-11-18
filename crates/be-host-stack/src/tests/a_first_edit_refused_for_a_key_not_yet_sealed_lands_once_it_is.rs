use super::*;

/// A grant to edit reaches this account before the block's key: the first edit is refused for
/// the missing key, the block shows locked while the edit waits, and the edit goes again and
/// lands once the key is sealed to this account, though its session never broke.
#[test]
fn a_first_edit_refused_for_a_key_not_yet_sealed_lands_once_it_is() {
    let harness = Harness::start();
    let sharer = Someone::register(
        &harness.directory.join("server"),
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
        .keys()
        .contact(harness.account)
        .expect("the sharer verified this account through the link");
    // Granted, its key not sealed yet: as a share's grant that reaches this account first.
    let block = runtime.block_on(async {
        let block = peer
            .create::<CounterContent>(be_block_graph::BlockParent::Root)
            .await
            .unwrap();
        peer.grant(block, harness.account, be_block_graph::Access::Edit)
            .await
            .unwrap();
        block
    });

    ask_for(vec![Query::Shared]);
    wait_until("listed the shared block as editable", |held| {
        held.graph
            .get(block)
            .is_some_and(|node| node.access.can_edit())
    });
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    wait_until("refused the edit for the missing key", |held| {
        held.locked.contains(&block)
    });

    runtime
        .block_on(peer.seal_share(block, harness.account, &public))
        .expect("the sharer seals the block's key to this account");
    wait_until("unlocked the block once its key was sealed", |held| {
        !held.locked.contains(&block)
    });
    flush_and_send();
    let saved = runtime.block_on(async {
        peer.open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(1), "the refused edit landed once its key came");
    assert_eq!(count_of(block), Some(1), "the edit landed once");
}
