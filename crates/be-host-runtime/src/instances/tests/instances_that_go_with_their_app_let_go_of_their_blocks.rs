use super::*;

use be_type_example_counter::CounterContent;

/// Instances that go with their app (a `Forwarded` the root lets go of) are each closed first
/// (`close_all`), and detach their editors as closing each would: the replica keeps no editor
/// that is gone, so the block, wanted by nothing else, closes and the stack holds it no longer.
#[test]
fn instances_that_go_with_their_app_let_go_of_their_blocks() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(to_replica(&mut instances, block, &FromEditor::Resend));
    replica_sent_until(&mut instances, block, |sent| {
        sent.iter()
            .any(|message| matches!(message, ToEditor::Snapshot(_)))
    });
    be_host_stack::wait_until("the editor's block held, in step with the server", |shared| {
        (shared.blocks.contains_key(&block)
            && shared.busy(true).is_empty()
            && shared.unsynced().is_empty())
        .then_some(())
    });

    instances.close_all();

    be_host_stack::wait_until("the block let go of", |shared| {
        (!shared.blocks.contains_key(&block)).then_some(())
    });
}
