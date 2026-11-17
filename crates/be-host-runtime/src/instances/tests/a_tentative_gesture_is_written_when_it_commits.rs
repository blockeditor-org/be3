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

/// An editor's tentative gesture, named by its id, is held while an edit outside it is written,
/// and written when it commits; one of its gestures cancelled meanwhile writes nothing
/// (plans/tentative_gestures.md).
#[test]
fn a_tentative_gesture_is_written_when_it_commits() {
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
    let gesture = |id, gesture| FromEditor::Gesture { id, gesture };
    assert!(to_replica(
        &mut instances,
        watched,
        &gesture(1, Gesture::BeginTentative)
    ));
    assert!(to_replica(
        &mut instances,
        watched,
        &gesture(2, Gesture::BeginTentative)
    ));
    assert!(to_replica(&mut instances, watched, &add(3, Some(1))));
    assert!(to_replica(&mut instances, watched, &add(7, Some(2))));
    assert!(to_replica(&mut instances, watched, &add(1, None)));
    counted(watched, 1);

    assert!(to_replica(
        &mut instances,
        watched,
        &gesture(2, Gesture::Cancel)
    ));
    assert!(to_replica(
        &mut instances,
        watched,
        &gesture(1, Gesture::Commit)
    ));
    counted(watched, 4);
    // Closing cancels no gesture the editor ended.
    assert!(instances.remove(INSTANCE));
    be_host_stack::flush_and_send();
    be_host_stack::open(watched, CounterContent::CONTENT_TYPE);
    counted(watched, 4);
}
