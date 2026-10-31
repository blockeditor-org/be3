use super::*;

#[test]
fn a_heartbeat_the_server_never_answers_holds_up_no_edit() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush();

    let mut heartbeats = harness
        .relay()
        .count(|message| matches!(message, be_protocol::ClientMessage::Heartbeat { .. }));
    let early = Duration::from_millis(block_type_guest::SEAL_INTERVAL / 2);
    harness
        .clock
        .advance(Duration::from_millis(block_type_guest::HEARTBEAT_INTERVAL) - early);
    harness.relay().stall();
    add(block, 2);
    wait_for_count(block, 3);
    harness.clock.advance(early);
    runtime()
        .block_on(heartbeats.wait_for(|beats| *beats > 0))
        .expect("the counter's owner sent a heartbeat");

    add(block, 4);
    wait_for_count(block, 7);
    assert!(
        status().connected,
        "the edit was taken while the heartbeat was still unanswered"
    );
}
