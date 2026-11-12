use super::*;

#[test]
fn an_owner_cut_off_before_it_commits_leaves_its_edits_to_the_next_owner() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush_and_send();

    let mut appends = harness
        .relay()
        .count(|method| method == be_block_protocol::__args::SESSIONS_SESSION_APPEND);
    add(block, 3);
    wait_for_count(block, 7);
    harness
        .clock
        .advance(Duration::from_millis(be_sdk_type_guest::APPEND_INTERVAL));
    let committed = runtime().block_on(async {
        appends
            .wait_for(|appends| *appends > 0)
            .await
            .expect("the owner appended its edit to the journal");
        let reader = harness.outside_peer().await;
        // The append's answer is not watched, so the reader asks until the server holds it.
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
    assert_eq!(saved, Some(7), "the new owner committed every edit");
    next.leave();
}
