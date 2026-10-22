use super::*;

use std::time::Duration;

use be_block::{BlockContent, Counter, CounterContent, LiveEdit};
use block_plugin_api::{GesturePhase, WatchedContent};

fn counted(block: Uuid, expected: i64) {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let counter = CounterContent::decode(held.bytes()).ok()?;
        (counter.root().value() == expected).then_some(())
    })
    .expect("the block never reached the expected count");
}

#[test]
fn closing_an_editor_calls_off_the_gesture_it_left_open() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let own = Uuid::new_v4();
    let watched = Uuid::new_v4();
    crate::be::create(
        watched,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(instances.editor_message(EditorMessage::WatchContent {
        instance: INSTANCE,
        blocks: vec![WatchedContent {
            block_id: watched.into_bytes(),
            content_type: CounterContent::CONTENT_TYPE.into_bytes(),
        }],
    }));
    next_screens(&mut instances);
    counted(watched, 0);

    let add = |amount| CounterContent::encode_operation(&Counter::add(amount));
    assert!(instances.editor_message(EditorMessage::Operate {
        instance: INSTANCE,
        block_id: watched.into_bytes(),
        operation: add(2),
    }));
    assert!(instances.editor_message(EditorMessage::Gesture {
        instance: INSTANCE,
        block_id: watched.into_bytes(),
        phase: GesturePhase::Begin,
    }));
    assert!(instances.editor_message(EditorMessage::Operate {
        instance: INSTANCE,
        block_id: watched.into_bytes(),
        operation: add(3),
    }));
    counted(watched, 5);

    assert!(instances.remove(INSTANCE));
    counted(watched, 2);
}
