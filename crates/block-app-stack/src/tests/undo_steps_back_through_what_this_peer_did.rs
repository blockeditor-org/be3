use super::*;

use be_block::Edit;
use checklist_types::{Checklist, ChecklistContent};

fn titles(shared: &Shared, block: Uuid) -> Option<Vec<String>> {
    let held = shared.blocks.get(&block)?;
    let checklist = ChecklistContent::decode(held.bytes()).ok()?;
    Some(
        checklist
            .root()
            .items
            .iter()
            .map(|item| item.text.clone())
            .collect(),
    )
}

fn reached(block: Uuid, expected: &[&str], history: History) {
    wait_until("reached the expected history", |shared| {
        titles(shared, block).is_some_and(|titles| titles == expected)
            && shared.histories.get(&block) == Some(&history)
    });
}

fn checklist(block: Uuid, edit: &Edit) {
    operate(block, ChecklistContent::encode_operation(edit));
}

#[test]
fn undo_steps_back_through_what_this_peer_did() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, ChecklistContent::CONTENT_TYPE);
    let (item, add) = Checklist::add("S");

    checklist(block, &add);
    checklist(block, &Checklist::set_text(item, "Standup"));
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
