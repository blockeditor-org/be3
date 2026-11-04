use super::*;

#[test]
fn closing_a_block_while_the_server_is_silent_holds_up_no_edit() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let closing = Uuid::new_v4();
    let editing = Uuid::new_v4();
    open(closing, CounterContent::CONTENT_TYPE);
    open(editing, CounterContent::CONTENT_TYPE);
    add(closing, 1);
    add(editing, 1);
    wait_for_count(closing, 1);
    wait_for_count(editing, 1);
    flush();

    harness.relay().stall();
    close(closing);
    wait_until("closed the counter", |shared| {
        !shared.blocks.contains_key(&closing)
    });
    add(editing, 2);
    wait_for_count(editing, 3);
    assert!(
        status().connected,
        "the edit was taken while leaving the closed block's session was still unanswered"
    );
}
