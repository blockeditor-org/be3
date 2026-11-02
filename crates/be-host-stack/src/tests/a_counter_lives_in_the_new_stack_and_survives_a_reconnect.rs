use super::*;

#[test]
fn a_counter_lives_in_the_new_stack_and_survives_a_reconnect() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 3);
    add(block, -1);
    wait_for_count(block, 2);

    stop();
    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);

    wait_for_count(block, 2);
}
