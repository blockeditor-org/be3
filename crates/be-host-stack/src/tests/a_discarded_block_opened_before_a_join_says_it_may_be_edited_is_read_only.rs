use super::*;

/// A block whose local edits were discarded is listed as editable again, but opened offline
/// before any join says so: its editors attach read-only, as nothing they made would be kept, so
/// an edit is refused rather than lost unseen. Once online, the join's word makes them editable.
#[test]
fn a_discarded_block_opened_before_a_join_says_it_may_be_edited_is_read_only() {
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

    harness.relay().offline(true);
    wait_until("noticed it is offline", |held| !held.connected);
    open(block, CounterContent::CONTENT_TYPE);
    let mut editor = Editor::<CounterContent>::attach(block);
    editor.wait("was sent the counter", |counter| counter.root().value() == 1);
    editor.operate(Counter::add(6));
    // The replica takes the editor's frames in order, and a flush waits for it to seal.
    flush();
    assert_eq!(
        count_of(block),
        Some(1),
        "an edit to a block whose edits were discarded is refused until a join says it may be edited"
    );

    editor.take_received();
    harness.relay().offline(false);
    retry_now();
    // Opened again on joining, the replica sends its editors their snapshot, now editable.
    editor.until("was sent the counter again by the replica the join opened", |editor| {
        editor.received.iter().any(|message| {
            matches!(message, be_sdk_type_guest::editor::ToEditor::Snapshot(_))
        })
    });
    editor.operate(Counter::add(10));
    wait_for_count(block, 11);
    flush_and_send();
    let saved = runtime.block_on(async {
        peer.open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(11), "only the edit made once it was editable landed");
}
