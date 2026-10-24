use super::*;

use std::time::Duration;

use be_block::{BlockContent, Counter, CounterContent, LiveEdit};
use block_plugin_api::WatchedContent;
use block_type_guest::editor::EditorOperation;

fn counted(block: Uuid, expected: i64) {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let counter = CounterContent::decode(held.bytes()).ok()?;
        (counter.root().value() == expected).then_some(())
    })
    .expect("the new stack never reached the expected count");
}

#[test]
fn an_editor_can_read_and_edit_a_block_it_watches() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let own = Uuid::new_v4();
    let other = Uuid::new_v4();
    crate::be::create(
        other,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(to_replica(&mut instances, own, &FromEditor::Resend));

    assert!(instances.editor_message(EditorMessage::WatchContent {
        instance: INSTANCE,
        blocks: vec![
            WatchedContent {
                block_id: other.into_bytes(),
                content_type: CounterContent::CONTENT_TYPE.into_bytes(),
            },
            WatchedContent {
                block_id: Uuid::new_v4().into_bytes(),
                content_type: Uuid::from_u128(0xdead_beef).into_bytes(),
            },
        ],
    }));
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

    assert!(instances.editor_message(EditorMessage::WatchContent {
        instance: INSTANCE,
        blocks: Vec::new(),
    }));
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        (!shared.blocks.contains_key(&other)).then_some(())
    })
    .expect("the watched block was never closed");
    assert!(crate::be::content(own).is_some());
}
