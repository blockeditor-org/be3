use super::*;

#[test]
fn a_holder_cut_off_before_it_seals_leaves_its_edits_to_the_next_holder() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush_and_send();

    let mut submits = harness.relay().count(|method, args| {
        method == be_block_protocol::__args::SERVER_SEND
            && be_wasm_ops::decode::<be_block_protocol::__args::ServerSendArgs>(args).is_some_and(
                |sent| {
                    matches!(
                        sent.target,
                        be_block_protocol::Target::Session {
                            kind: be_block_protocol::SessionSend::Ordered(_),
                            ..
                        }
                    )
                },
            )
    });
    add(block, 3);
    wait_for_count(block, 7);
    let committed = runtime().block_on(async {
        submits
            .wait_for(|submits| *submits > 0)
            .await
            .expect("the holder submitted its edit to the server's order");
        let reader = harness.outside_peer().await;
        // The submission's answer is not watched, so the reader asks until the server holds it.
        while !reader
            .read_contents(&[(block, None)])
            .await
            .expect("the block reads")
            .into_iter()
            .any(|read| {
                read.journal
                    .is_some_and(|journal| !journal.entries.is_empty())
            })
        {}
        reader
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens")
            .map(|counter| counter.root().value())
    });
    assert_eq!(
        committed,
        Some(4),
        "the edit is in the journal, not yet in a commit"
    );

    harness.relay().hold_new(true);
    harness.relay().cut();
    wait_until("noticed the connection was cut", |shared| !shared.connected);

    let mut next = Outside::joining(&harness, block);
    next.until("replayed the journal", |seen| seen.count == Some(7));
    next.claim();
    next.seal();
    let saved = next.block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(7), "the next holder sealed every edit");
    next.leave();
}
