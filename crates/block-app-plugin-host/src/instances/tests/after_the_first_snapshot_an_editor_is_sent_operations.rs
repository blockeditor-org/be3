use super::*;

use std::time::Duration;

use be_block::LiveEdit;
use block_type_guest::editor::EditorOperation;
use counter_types::{Counter, CounterContent};

fn counted(block: Uuid, expected: i64) {
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let counter = <CounterContent as be_block::BlockContent>::decode(held.bytes()).ok()?;
        (counter.root().value() == expected).then_some(())
    })
    .expect("the new stack never reached the expected count");
}

fn operations(sent: &[ToEditor]) -> Vec<EditorOperation> {
    sent.iter()
        .flat_map(|message| match message {
            ToEditor::Operations(operations) => operations.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn after_the_first_snapshot_an_editor_is_sent_operations() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    block_app_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(to_replica(&mut instances, block, &FromEditor::Resend));
    let sent = replica_sent_until(&mut instances, block, |sent| {
        sent.iter()
            .any(|message| matches!(message, ToEditor::Snapshot(_)))
    });
    assert!(operations(&sent).is_empty());

    let own = CounterContent::encode_operation(&Counter::add(2));
    assert!(to_replica(
        &mut instances,
        block,
        &FromEditor::Operate(own.clone())
    ));
    counted(block, 2);
    let sent = replica_sent_until(&mut instances, block, |sent| !operations(sent).is_empty());
    assert_eq!(
        operations(&sent),
        [EditorOperation {
            operation: own,
            mine: true,
            unsettled: 0,
            beneath: None,
        }]
    );

    let elsewhere = CounterContent::encode_operation(&Counter::add(5));
    block_app_stack::operate(block, elsewhere.clone());
    counted(block, 7);
    let sent = replica_sent_until(&mut instances, block, |sent| !operations(sent).is_empty());
    assert_eq!(
        operations(&sent),
        [EditorOperation {
            operation: elsewhere,
            mine: false,
            unsettled: 0,
            beneath: None,
        }]
    );
    assert!(replica_sends(&mut instances, block).is_empty());
}
