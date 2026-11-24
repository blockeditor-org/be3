use super::*;

/// A block whose replica found its head's objects not to be had (here the server went away as
/// it read them) waits to open again, and exporting it fails at once with "not available yet"
/// rather than read what is stored or wait.
#[test]
fn a_block_whose_replica_waits_for_its_heads_objects_does_not_export() {
    let harness = Harness::relayed().on_manual_clock();
    let block = Uuid::new_v4();
    let other = runtime();
    other.block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .ensure::<CounterContent>(block, be_block_graph::BlockParent::Root)
            .await
            .expect("the block is created");
        let mut counter = CounterContent::default();
        counter.apply(&Counter::add(1));
        outside
            .save(block, &counter, None)
            .await
            .expect("the head is saved");
    });

    harness.connect();
    wait_until("knew the block's head", |shared| {
        shared.graph.get(block).is_some_and(|node| node.head.is_some())
    });
    // Its join is slow, so it opens on the head it knows, whose objects it asks the server for;
    // the server goes before it answers.
    harness.relay().hold_answers(true);
    let mut reads = harness.relay().count(|method, _| {
        method == be_block_protocol::__args::SERVER_READ
    });
    let before = *reads.borrow();
    hold(block, CounterContent::CONTENT_TYPE);
    runtime()
        .block_on(reads.wait_for(|reads| *reads > before))
        .expect("holding the block asked to join its session");
    let joined = *reads.borrow();
    harness.clock.advance(be_block_type_host::EARLY_OPEN);
    runtime()
        .block_on(reads.wait_for(|reads| *reads > joined))
        .expect("the replica asked for its head's objects");
    harness.relay().offline(true);

    // Each export goes through the worker, after whatever it was told before: one asked as the
    // read fails may still find the server gone.
    let exported = (0..10_000)
        .find_map(|_| {
            let (sender, receiver) = std::sync::mpsc::channel();
            export(block, move |exported| {
                let _ = sender.send(exported);
            });
            match receiver.recv().expect("the export answers") {
                Err(error) if error.contains("not available yet") => Some(error),
                _ => None,
            }
        })
        .expect("the export failed as not available yet");
    assert!(exported.contains("not available yet"), "{exported}");
}
