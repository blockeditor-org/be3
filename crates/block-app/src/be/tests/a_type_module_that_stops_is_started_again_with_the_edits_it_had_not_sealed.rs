use super::*;

#[test]
fn a_type_module_that_stops_is_started_again_with_the_edits_it_had_not_sealed() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush();
    let sealed = wait_for(PATIENCE, |shared| shared.graph.get(block)?.head)
        .expect("the first add was sealed");

    add(block, 3);
    wait_for_count(block, 5);
    super::types::CRASH.store(true, std::sync::atomic::Ordering::SeqCst);
    add(block, 4);
    wait_for_count(block, 9);
    flush();
    let head = wait_for(PATIENCE, |shared| {
        let head = shared.graph.get(block)?.head?;
        (head != sealed && shared.owing == 0).then_some(head)
    })
    .expect("the module, started again, sealed what it had replayed");
    let saved = runtime().block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .open_commit::<CounterContent>(head)
            .await
            .expect("the sealed commit opens")
    });
    assert_eq!(saved.root().value(), 9);
}
