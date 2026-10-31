use super::*;

#[test]
fn an_idle_peer_never_wakes_its_worker() {
    let harness = Harness::start().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    flush();
    let (settled, armed) =
        crate::wait_until(|shared| shared.parked.map(|armed| (shared.wakes, armed)));
    assert_eq!(armed, None, "the idle worker set a timer to wake itself");

    let mut due = crate::wait_until(|shared| {
        shared
            .connected
            .then(|| shared.connection_due.clone())
            .flatten()
    });
    let lapse = 2 * crate::worker::OUTBOX_RETRY
        .max(Duration::from_millis(block_type_guest::HEARTBEAT_INTERVAL))
        .max(Duration::from_millis(block_type_guest::SEAL_INTERVAL));
    let until = harness.clock.now() + lapse;
    while harness.clock.now() < until {
        let at = *due.borrow_and_update();
        harness
            .clock
            .advance(at.saturating_sub(harness.clock.now()));
        runtime()
            .block_on(due.wait_for(|next| *next == at + be_client::connection::PING_INTERVAL))
            .expect("the connection answers its pings");
    }

    let woke = crate::wait_until(|shared| shared.parked.map(|_| shared.wakes));
    assert_eq!(
        woke, settled,
        "the worker woke without being asked while the connection pinged"
    );
}
