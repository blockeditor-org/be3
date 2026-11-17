use super::*;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::Gesture;
use be_type_example_counter::{Counter, CounterContent};

fn counted(block: Uuid, expected: i64) {
    be_host_stack::wait_until(&format!("counted {expected}"), |shared| {
        let held = shared.blocks.get(&block)?;
        let counter = CounterContent::decode(held.bytes()).ok()?;
        (counter.root().value() == expected).then_some(())
    });
}

/// The tentative gestures an editor left open when it goes, two here (two fingers), are each
/// cancelled, and their held edits are never written (plans/tentative_gestures.md).
#[test]
fn closing_an_editor_drops_the_tentative_gesture_it_left_open() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let own = Uuid::new_v4();
    let watched = Uuid::new_v4();
    be_host_stack::create(
        watched,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    hold(&mut instances, watched, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    assert!(to_replica(&mut instances, watched, &FromEditor::Resend));
    counted(watched, 0);

    let add = |amount, gesture| FromEditor::Operate {
        gesture,
        operation: CounterContent::encode_operation(&Counter::add(amount)),
    };
    for id in [4, 9] {
        assert!(to_replica(
            &mut instances,
            watched,
            &FromEditor::Gesture {
                id,
                gesture: Gesture::BeginTentative
            }
        ));
    }
    assert!(to_replica(&mut instances, watched, &add(3, Some(4))));
    assert!(to_replica(&mut instances, watched, &add(5, Some(9))));
    assert!(to_replica(&mut instances, watched, &add(2, None)));
    // Held: shown to the editors, but never written, so closing leaves no trace of them.
    counted(watched, 2);
    assert!(instances.remove(INSTANCE));
    be_host_stack::flush_and_send();
    be_host_stack::open(watched, CounterContent::CONTENT_TYPE);
    counted(watched, 2);
}
