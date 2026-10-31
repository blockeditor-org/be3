use super::*;

use be_block::{BlockContent, LiveEdit};
use block_type_guest::editor::EditorOperation;
use counter_types::{Counter, CounterContent};

fn counted(block: Uuid, expected: i64) {
    block_app_stack::wait_until(|shared| {
        let held = shared.blocks.get(&block)?;
        let counter = CounterContent::decode(held.bytes()).ok()?;
        (counter.root().value() == expected).then_some(())
    });
}

#[test]
fn an_editor_can_read_and_edit_a_block_it_watches() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let own = Uuid::new_v4();
    let other = Uuid::new_v4();
    block_app_stack::create(
        other,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(to_replica(&mut instances, own, &FromEditor::Resend));

    let watched = hold(&mut instances, other, CounterContent::CONTENT_TYPE);
    let unknown = hold(&mut instances, Uuid::new_v4(), Uuid::from_u128(0xdead_beef));
    assert!(to_replica(&mut instances, other, &FromEditor::Resend));
    counted(other, 0);
    let sent = replica_sent_until(&mut instances, other, |sent| !sent.is_empty());
    assert!(matches!(sent.as_slice(), [ToEditor::Snapshot(_)]));

    let add = CounterContent::encode_operation(&Counter::add(3));
    assert!(to_replica(
        &mut instances,
        other,
        &FromEditor::Operate(add.clone())
    ));
    counted(other, 3);
    let sent = replica_sent_until(&mut instances, other, |sent| !sent.is_empty());
    assert_eq!(
        sent,
        [ToEditor::Operations(vec![EditorOperation {
            operation: add,
            mine: true,
            unsettled: 0,
            beneath: None,
        }])]
    );

    instances.ended(watched);
    instances.ended(unknown);
    block_app_stack::wait_until(|shared| (!shared.blocks.contains_key(&other)).then_some(()));
    assert!(block_app_stack::content(own).is_some());
}
