use super::*;

#[test]
fn an_edit_in_a_block_opened_offline_is_not_delivered_until_it_is_published() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let address = harness.address();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush();
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    stop();

    harness.connect();
    assert!(!status().connected);
    open(block, CounterContent::CONTENT_TYPE);
    let mut editor = Editor::<CounterContent>::attach(block);
    editor.wait("was sent the counter", |counter| {
        counter.root().value() == 2
    });
    editor.operate(Counter::add(4));
    editor.until("the edit was confirmed", |editor| {
        editor.mirror.pending() == 0
    });
    flush();
    editor.pump();
    assert_eq!(
        editor.mirror.delivered(),
        0,
        "the first edit in a block opened offline is still sending"
    );

    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and sent what waited", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    editor.until("delivered the edit once it was published", |editor| {
        editor.mirror.delivered() == 1
    });
}
