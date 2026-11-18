use super::*;
use be_block_content::BlockContent;
use be_type_example_counter::CounterContent;

/// An editor holds two replicas of its block: the frames go to the later, and once it ends, the
/// earlier hears a snapshot before anything else, so it never applies what it missed.
#[test]
fn an_earlier_replica_of_a_block_resumes_from_a_snapshot() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let block_type = CounterContent::CONTENT_TYPE;
    let mut instances = placed_on(block, block_type);
    next_screens(&mut instances);
    let earlier = replica_of(&mut instances, block).expect("the editor's own block");
    to_replica(&mut instances, block, &FromEditor::Resend);
    be_host_stack::wait_until("held the block", |shared| {
        shared.blocks.contains_key(&block).then_some(())
    });
    be_host_stack::wait_for_frames("the earlier replica's content", INSTANCE.0);
    assert!(!from_replica(&mut instances, block).is_empty());

    let later = hold(&mut instances, block, block_type);
    assert_eq!(
        instances
            .entries
            .get(&INSTANCE)
            .and_then(|entry| entry.held.replica(block)),
        Some(later),
        "frames go to the later replica"
    );
    instances.ended(later);
    assert_eq!(
        instances
            .entries
            .get(&INSTANCE)
            .and_then(|entry| entry.held.replica(block)),
        Some(earlier)
    );
    be_host_stack::wait_for_frames("the earlier replica's snapshot", INSTANCE.0);
    let resumed = from_replica(&mut instances, block);
    assert!(
        matches!(resumed.first(), Some(ToEditor::Snapshot(_))),
        "the earlier replica hears a snapshot first: {resumed:?}"
    );
}
