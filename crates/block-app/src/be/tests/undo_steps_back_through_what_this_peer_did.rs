use super::*;

use be_block::{CalendarContent, CalendarEvent, CalendarOp};

fn titles(shared: &Shared, block: Uuid) -> Option<Vec<String>> {
    let held = shared.blocks.get(&block)?;
    let calendar = CalendarContent::decode(&held.bytes).ok()?;
    Some(
        calendar
            .events()
            .iter()
            .map(|event| event.title.clone())
            .collect(),
    )
}

fn reached(block: Uuid, expected: &[&str], history: History) {
    wait_until("reached the expected history", |shared| {
        titles(shared, block).is_some_and(|titles| titles == expected)
            && shared.histories.get(&block) == Some(&history)
    });
}

fn calendar(block: Uuid, operation: &CalendarOp) {
    operate(block, CalendarContent::encode_operation(operation));
}

#[test]
fn undo_steps_back_through_what_this_peer_did() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CalendarContent::CONTENT_TYPE);
    let event = CalendarEvent::new("S".to_owned(), 540, 555);

    calendar(
        block,
        &CalendarOp::AddEvent {
            event: event.clone(),
        },
    );
    calendar(
        block,
        &CalendarOp::UpdateEvent {
            event: CalendarEvent {
                title: "Standup".to_owned(),
                ..event.clone()
            },
        },
    );
    let only_undo = History {
        can_undo: true,
        can_redo: false,
    };
    reached(block, &["Standup"], only_undo);

    undo(block);
    reached(
        block,
        &["S"],
        History {
            can_undo: true,
            can_redo: true,
        },
    );
    undo(block);
    reached(
        block,
        &[],
        History {
            can_undo: false,
            can_redo: true,
        },
    );
    redo(block);
    redo(block);
    reached(block, &["Standup"], only_undo);
}
