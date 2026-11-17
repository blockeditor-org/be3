use super::*;

/// Edits made to an open block whose edit access was taken away meanwhile are set aside among
/// the unsent edits, and its replica, rejoined as a viewer's, shows what the server holds. What
/// is typed on is refused and given up rather than shown as sending for ever. Discarded, the
/// unsent edits take the edits this device kept of the block with them, and once its replica
/// closes nothing keeps the block from being forgotten.
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
    // Rejoined as a viewer, the replica follows the server's order, which holds none of what
    // was set aside: that shows among the unsent edits instead, until it is discarded. Until it
    // rejoins, its made-up session shows 3.
    wait_for_count(block, 1);
    // Typed on, as the host still operates on it: the server refuses the edit, as this account
    // may not edit the block, and the replica gives it up, showing what the server holds and
    // owing nothing.
    add(block, 4);
    wait_until("gave up the edit the server refused", |held| {
        held.pending() == 0 && held.unsealed == 0 && counted(held, block) == Some(1)
    });
    // And kept nothing of it to give a replica opened again, which would send it again.
    wait_until("kept none of the refused edit", |held| {
        held.peer.as_ref().is_some_and(|peer| {
            peer.open_edits(block)
                .is_none_or(|(kept, _)| kept.edits.is_empty() && kept.submissions.is_empty())
        })
    });
    crate::discard_unsent(block);
    wait_until("discarded the unsent edits", |held| {
        !held.unsent.iter().any(|unsent| unsent.block == block)
    });
    crate::close(block);
    flush();
    // A viewer's replica seals nothing as it closes: nothing is set aside again.
    wait_until("closed it", |held| {
        held.waiting == 0 && !held.blocks.contains_key(&block)
    });
    assert!(!crate::unsent().iter().any(|unsent| unsent.block == block));
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
