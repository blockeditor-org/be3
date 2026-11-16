use super::*;

/// What is typed into a block after this account was made a viewer is discarded with the
/// block's unsent edits: once the account may edit the block again, reopening it shows only
/// what the server holds, and what was typed meanwhile does not come back with the next edit.
#[test]
fn edits_typed_after_a_demotion_stay_discarded_once_the_block_is_editable_again() {
    let harness = Harness::relayed();
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
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .verified_identity(harness.account)
        .expect("the sharer verified this account through the link");
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

    ask_for(vec![Query::Shared]);
    wait_until("listed the shared block", |held| {
        held.graph
            .get(block)
            .is_some_and(|node| node.access.can_edit())
    });
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    harness.relay().offline(true);
    wait_until("noticed it is offline", |held| !held.connected);
    add(block, 2);
    wait_for_count(block, 3);
    flush();
    runtime
        .block_on(peer.grant(block, harness.account, be_block_graph::Access::View))
        .expect("the sharer makes this account a viewer");

    harness.relay().offline(false);
    retry_now();
    wait_until("set the edits made meanwhile aside", |held| {
        held.unsent
            .iter()
            .any(|unsent| unsent.block == block && unsent.refused.is_none())
    });
    // Typed while it is open, after the demotion: taken by a replica that has yet to hear of
    // it, or refused by one that has.
    add(block, 6);
    wait_until("took or refused the edit typed after the demotion", |held| {
        counted(held, block) == Some(9)
            || held
                .error
                .as_ref()
                .is_some_and(|error| error.contains("not editable"))
    });
    // Discarded before anything seals it: the replica holds it as the discard closes it.
    crate::discard_unsent(block);
    wait_until("discarded the unsent edits and closed the block", |held| {
        !held.unsent.iter().any(|unsent| unsent.block == block) && !held.blocks.contains_key(&block)
    });
    runtime
        .block_on(peer.grant(block, harness.account, be_block_graph::Access::Edit))
        .expect("the sharer makes this account an editor again");
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 1);
    add(block, 10);
    wait_for_count(block, 11);
    refetch(Query::Shared);
    wait_until("listed the block as editable again", |held| {
        held.graph
            .get(block)
            .is_some_and(|node| node.access.can_edit())
    });
    wait_until("had the edit held by the server", |held| {
        held.undurable == 0
    });

    flush_and_send();
    assert_eq!(
        count_of(block),
        Some(11),
        "what was typed after the demotion stayed discarded"
    );
    let saved = runtime.block_on(async {
        peer.open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(11), "the server holds only what was sent");
}
