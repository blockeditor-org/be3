use super::*;

/// Edits made to an open block whose edit access was taken away meanwhile are set aside among
/// the unsent edits. Discarded, they take the edits this device kept of the block with them,
/// and its open replica closes: what it made after is sealed as it closes and set aside in its
/// turn, and once that is discarded too nothing keeps the block from being forgotten.
#[test]
fn discarding_the_unsent_edits_of_a_block_demoted_mid_edit_leaves_nothing_of_it_kept() {
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
    // Nothing keeps it: what it still lists of the block, which it may view, is learned again.
    wait_until("kept nothing that keeps the block", |held| {
        held.peer.as_ref().is_some_and(|peer| {
            peer.open_edits(block).is_none()
                && peer
                    .local_journal(block)
                    .ok()
                    .flatten()
                    .is_none_or(|journal| journal.entries.is_empty())
                && peer
                    .forget_cached(block)
                    .is_ok_and(|forgotten| forgotten >= 1)
        })
    });
}
