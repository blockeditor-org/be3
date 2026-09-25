use super::*;

#[test]
fn a_duplicated_block_carries_what_its_source_held() {
    let harness = Harness::start();
    harness.connect();
    let source = Uuid::new_v4();
    open(source, CounterContent::CONTENT_TYPE);
    add(source, 5);
    wait_for_count(source, 5);

    wait_until("heard the source join the graph", |shared| {
        shared.graph.get(source).is_some()
    });
    let (while_open, _) = duplicate(source).expect("the source is in the graph");
    open(while_open, CounterContent::CONTENT_TYPE);
    wait_for_count(while_open, 5);

    close(source);
    let (after_closing, _) = duplicate(source).expect("the source is in the graph");
    open(after_closing, CounterContent::CONTENT_TYPE);
    wait_for_count(after_closing, 5);

    add(while_open, 1);
    wait_for_count(while_open, 6);
    wait_for_count(after_closing, 5);
}
