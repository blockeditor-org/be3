use super::*;

/// Offline, this device's clock jumps two hours ahead and an edit of an open counter is sealed
/// by it, past what the server takes. Reconnected, the drain stamps the commit again
/// (`Peer::restamp`), which gives it a new id. The replica still open on the old one must take
/// the new one as its own, not merge it in as another device's head, which would count the
/// edit twice (a count of 8).
#[test]
fn an_edit_sealed_offline_by_a_fast_clock_is_stamped_again_and_counted_once() {
    let harness = Harness::relayed();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush_and_send();

    harness.relay().offline(true);
    wait_until("noticed the server was gone", |shared| !shared.connected);
    // A pinned clock is read with the offset this device last measured, as after a restart
    // offline: two hours fast.
    let fast = std::time::Duration::from_millis(
        u64::try_from(be_block_commit::now_milliseconds() + 2 * be_block_commit::HOUR)
            .expect("the clock reads after 1970"),
    );
    crate::pin_wall_clock(Some(fast));
    add(block, 3);
    wait_for_count(block, 5);
    flush();
    wait_until("sealed the edit offline", |shared| shared.waiting > 0);
    crate::pin_wall_clock(None);

    harness.relay().offline(false);
    retry_now();
    wait_until("reconnected and published the edit", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    assert_eq!(status().error, None);
    flush_and_send();
    assert_eq!(count_of(block), Some(5));

    let (saved, time) = runtime().block_on(async {
        let outside = harness.outside_peer().await;
        let head = outside
            .remote_head(block)
            .await
            .expect("the server answers")
            .expect("the counter has a head");
        let time = outside
            .load_reached(head)
            .await
            .expect("the head loads")
            .time;
        let saved = outside
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value());
        (saved, time)
    });
    assert_eq!(saved, Some(5));
    assert!(
        time <= be_block_commit::now_milliseconds() + be_block_commit::MAX_COMMIT_AHEAD,
        "the head kept its far-future time"
    );
}
