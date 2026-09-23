use super::*;

use std::time::Duration;

use be_block::{Counter, CounterContent, LiveEdit};
use block::Block;
use block_plugin_api::ContentOperation;

fn counted(block: Uuid, expected: i64) {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let counter = <CounterContent as be_block::BlockContent>::decode(&held.bytes).ok()?;
        (counter.root().value() == expected).then_some(())
    })
    .expect("the new stack never reached the expected count");
}

fn content_messages(instances: &mut Instances) -> Vec<EditorMessage> {
    instances
        .next_screens(PASS)
        .opened
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(
                editor @ (EditorMessage::Content { .. } | EditorMessage::ContentOperations { .. }),
            ) => Some(editor),
            _ => None,
        })
        .collect()
}

#[test]
fn after_the_first_snapshot_an_editor_is_sent_operations() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed_on(block, block_client::blocks::counter::Counter::TYPE_ID);
    instances.next_screens(PASS);
    counted(block, 0);
    assert!(matches!(
        content_messages(&mut instances).as_slice(),
        [EditorMessage::Content { .. }]
    ));

    let own = CounterContent::encode_operation(&Counter::add(2));
    assert!(instances.editor_message(EditorMessage::Operate {
        instance: INSTANCE,
        block_id: block.into_bytes(),
        operation: own.clone(),
    }));
    counted(block, 2);
    assert_eq!(
        content_messages(&mut instances),
        [EditorMessage::ContentOperations {
            instance: INSTANCE,
            block_id: block.into_bytes(),
            operations: vec![ContentOperation {
                operation: own,
                mine: true,
            }],
        }]
    );

    let elsewhere = CounterContent::encode_operation(&Counter::add(5));
    crate::be::operate_from(block, 0, elsewhere.clone());
    counted(block, 7);
    assert_eq!(
        content_messages(&mut instances),
        [EditorMessage::ContentOperations {
            instance: INSTANCE,
            block_id: block.into_bytes(),
            operations: vec![ContentOperation {
                operation: elsewhere,
                mine: false,
            }],
        }]
    );
    assert!(content_messages(&mut instances).is_empty());
}
