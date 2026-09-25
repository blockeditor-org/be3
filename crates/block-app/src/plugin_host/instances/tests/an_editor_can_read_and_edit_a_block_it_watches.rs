use super::*;

use std::time::Duration;

use be_block::{BlockContent, Counter, CounterContent, LiveEdit};
use block_plugin_api::{ContentOperation, WatchedContent};

fn counted(block: Uuid, expected: i64) {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let counter = CounterContent::decode(&held.bytes).ok()?;
        (counter.root().value() == expected).then_some(())
    })
    .expect("the new stack never reached the expected count");
}

fn about(instances: &mut Instances, block: Uuid) -> Vec<EditorMessage> {
    instances
        .next_screens(PASS)
        .opened
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(
                editor @ (EditorMessage::Content { block_id, .. }
                | EditorMessage::ContentOperations { block_id, .. }),
            ) if block_id == block.into_bytes() => Some(editor),
            _ => None,
        })
        .collect()
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
    instances.next_screens(PASS);

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
    instances.next_screens(PASS);
    counted(other, 0);
    assert!(matches!(
        about(&mut instances, other).as_slice(),
        [EditorMessage::Content { .. }]
    ));

    let add = CounterContent::encode_operation(&Counter::add(3));
    assert!(instances.editor_message(EditorMessage::Operate {
        instance: INSTANCE,
        block_id: other.into_bytes(),
        operation: add.clone(),
    }));
    counted(other, 3);
    assert_eq!(
        about(&mut instances, other),
        [EditorMessage::ContentOperations {
            instance: INSTANCE,
            block_id: other.into_bytes(),
            operations: vec![ContentOperation {
                operation: add,
                mine: true,
            }],
        }]
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
