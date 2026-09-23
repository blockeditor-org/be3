use super::*;

use block::Block;
use block_client::blocks::counter::Counter;

#[test]
fn a_duplicated_block_carries_what_its_source_held() {
    let harness = Harness::start();
    harness.connect();
    let source = Uuid::new_v4();
    open(source, CounterContent::CONTENT_TYPE);
    add(source, 5);
    wait_for_count(source, 5);

    let while_open = Uuid::new_v4();
    duplicate(source, while_open, Counter::TYPE_ID);
    open(while_open, CounterContent::CONTENT_TYPE);
    wait_for_count(while_open, 5);

    close(source);
    let after_closing = Uuid::new_v4();
    duplicate(source, after_closing, Counter::TYPE_ID);
    open(after_closing, CounterContent::CONTENT_TYPE);
    wait_for_count(after_closing, 5);

    add(while_open, 1);
    wait_for_count(while_open, 6);
    wait_for_count(after_closing, 5);
}
