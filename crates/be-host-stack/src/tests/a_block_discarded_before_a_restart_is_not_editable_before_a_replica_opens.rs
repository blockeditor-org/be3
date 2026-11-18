use super::*;

/// A block whose local edits were discarded stays marked across a restart: before any replica of
/// it opens, the mark the refs database kept caps its access at `View`, so graph commands
/// (`Instances::editable`) answer as they did before the restart.
#[test]
fn a_block_discarded_before_a_restart_is_not_editable_before_a_replica_opens() {
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
            &Default::default(),
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
    crate::discard_unsent(block);
    wait_until("discarded the unsent edits and closed the block", |held| {
        !held.unsent.iter().any(|unsent| unsent.block == block) && !held.blocks.contains_key(&block)
    });
    runtime
        .block_on(peer.grant(block, harness.account, be_block_graph::Access::Edit))
        .expect("the sharer makes this account an editor again");
    refetch(Query::Shared);
    wait_until("listed the block as editable again", |held| {
        held.graph
            .get(block)
            .is_some_and(|node| node.access.can_edit())
    });

    assert!(
        !crate::access(block).can_edit(),
        "a discarded block is not editable before the restart"
    );

    harness.relay().offline(true);
    stop();
    harness.start_in(personal);
    wait_until("listed the block as editable from what it kept", |held| {
        held.graph
            .get(block)
            .is_some_and(|node| node.access.can_edit())
    });
    assert!(
        !held_blocks_contain(block),
        "no replica of the discarded block opened"
    );
    assert!(
        !crate::access(block).can_edit(),
        "a block discarded before the restart is not editable before a replica opens"
    );
}

fn held_blocks_contain(block: Uuid) -> bool {
    crate::with_seen(|held| held.blocks.contains_key(&block)).unwrap_or_default()
}
