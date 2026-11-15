use super::*;

/// A block whose local edits were discarded once this account was made a viewer is made
/// editable again while this device still lists it as a viewer's: an edit made then waits
/// until the device keeps it, as it may edit the block again, rather than going out unkept,
/// and survives a crash to land once.
#[test]
fn an_edit_to_a_block_made_editable_again_after_its_edits_were_discarded_lands_once_across_a_crash() {
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
        .keys()
        .contact(harness.account)
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
    // Typed on, as the replica still takes edits it can no longer send.
    add(block, 4);
    wait_for_count(block, 7);
    crate::discard_unsent(block);
    wait_until("discarded the unsent edits", |held| {
        !held.unsent.iter().any(|unsent| unsent.block == block)
    });
    crate::close(block);
    flush();
    // What the replica made after the edits it was given was sealed as it closed, and set aside
    // in its turn unless it went with them: shown, never kept out of sight.
    wait_until("closed it and sent or set aside what it made", |held| {
        held.waiting == 0 && !held.blocks.contains_key(&block)
    });
    if crate::unsent().iter().any(|unsent| unsent.block == block) {
        crate::discard_unsent(block);
        wait_until("discarded it", |held| {
            !held.unsent.iter().any(|unsent| unsent.block == block)
        });
    }
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
    wait_until("had the edit held by the server", |held| held.undurable == 0);

    crash();
    platform::wait_for_stopped();
    harness.start_in(personal);
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 11);
    flush_and_send();
    assert_eq!(count_of(block), Some(11), "the edit landed once");
}
