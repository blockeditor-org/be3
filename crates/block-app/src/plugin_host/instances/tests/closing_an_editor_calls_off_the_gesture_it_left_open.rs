use super::*;

use std::time::Duration;

use be_block::{BlockContent, LiveEdit};
use block_type_guest::editor::Gesture;
use counter_types::{Counter, CounterContent};

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
    hold(&mut instances, watched, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(to_replica(&mut instances, watched, &FromEditor::Resend));
    counted(watched, 0);

    let add = |amount| CounterContent::encode_operation(&Counter::add(amount));
    assert!(to_replica(
        &mut instances,
        watched,
        &FromEditor::Operate(add(2))
    ));
    assert!(to_replica(
        &mut instances,
        watched,
        &FromEditor::Gesture(Gesture::Begin)
    ));
    assert!(to_replica(
        &mut instances,
        watched,
        &FromEditor::Operate(add(3))
    ));
    counted(watched, 5);

    assert!(instances.remove(INSTANCE));
    crate::be::flush();
    crate::be::open(watched, CounterContent::CONTENT_TYPE);
    counted(watched, 2);
}
