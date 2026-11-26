use super::*;

use be_type_example_counter::CounterContent;

fn new_counter() -> Uuid {
    let block = Uuid::new_v4();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    block
}

/// Asks the replicas of `blocks` for the editor's content, as an app that starts does, and
/// waits for each snapshot to reach it.
fn resend_and_take_snapshots(instances: &mut Instances, blocks: [Uuid; 2]) {
    for block in blocks {
        assert!(to_replica(instances, block, &FromEditor::Resend));
    }
    let mut snapshotted = [false; 2];
    loop {
        next_screens(instances);
        let replicas = blocks.map(|block| {
            instances
                .entries
                .get(&INSTANCE)
                .and_then(|entry| entry.held.replica(block))
        });
        instances.relay(|op, frame| {
            let message = be_host_stack::unframe(frame).and_then(|bytes| ToEditor::decode(&bytes));
            if let Some(at) = replicas.iter().position(|replica| *replica == Some(op)) {
                snapshotted[at] |= matches!(message, Some(ToEditor::Snapshot(_)));
            }
            true
        });
        if snapshotted == [true; 2] {
            return;
        }
        be_host_stack::wait_for_frames("the snapshots for the editor", INSTANCE.0);
    }
}

/// How many times each link's content reached the editor after it lacked it, own block first.
fn deliveries(instances: &Instances, [own, watched]: [Uuid; 2]) -> (u64, u64) {
    let entry = instances
        .entries
        .get(&INSTANCE)
        .expect("the instance is open");
    assert_eq!(entry.role.block().map(|block| block.id), Some(own));
    let own = entry.content.as_ref().expect("its block is linked");
    let watched = entry
        .watched
        .get(&watched)
        .expect("the block it holds is linked");
    (own.deliveries, watched.deliveries)
}

fn awaits_content(instances: &Instances) -> bool {
    instances
        .entries
        .get(&INSTANCE)
        .is_some_and(Instance::awaits_content)
}

/// An app that restarts with its editor attached to its own block and to a block it holds is
/// sent each one's snapshot again, and its content counts as delivered: nothing waits for it.
/// The link stays attached across the restart (`Instances::reopen`), so the new run's `Resend`
/// reaches the replica, which answers it; were it unattached while the replica kept the editor,
/// attaching it again would send nothing (`Replicas::attach`) and the resend would not be
/// forwarded (`replica_frame`).
#[test]
fn a_restarted_apps_editor_is_sent_its_snapshot_and_counts_it_delivered() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let own = new_counter();
    let watched = new_counter();
    let blocks = [own, watched];
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    hold(&mut instances, watched, CounterContent::CONTENT_TYPE);
    resend_and_take_snapshots(&mut instances, blocks);
    assert!(!awaits_content(&instances), "the first run has its content");
    assert_eq!(deliveries(&instances, blocks), (1, 1));

    restart(&mut instances);
    assert!(
        awaits_content(&instances),
        "the new run starts without its content"
    );
    next_screens(&mut instances);
    hold(&mut instances, watched, CounterContent::CONTENT_TYPE);
    resend_and_take_snapshots(&mut instances, blocks);

    assert!(
        !awaits_content(&instances),
        "a restarted app's content counts as delivered once its snapshots reach it"
    );
    assert_eq!(
        deliveries(&instances, blocks),
        (2, 2),
        "each block's content reached the new run"
    );
}
