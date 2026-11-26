use super::*;

use be_block_content::Edit;
use be_type_checklist::{Checklist, ChecklistContent};

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

/// An edit as an editor makes it: the host's own (`operate`) are not this peer's to undo.
fn checklist(editor: &mut Editor<ChecklistContent>, edit: &Edit) {
    editor.send(be_sdk_type_guest::editor::FromEditor::Operate {
        gesture: None,
        operation: ChecklistContent::encode_operation(edit),
    });
}

#[test]
fn undo_steps_back_through_what_this_peer_did() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, ChecklistContent::CONTENT_TYPE);
    let mut editor = Editor::<ChecklistContent>::attach(block);
    let (item, add) = Checklist::add("S");

    checklist(&mut editor, &add);
    checklist(&mut editor, &Checklist::set_text(item, "Standup"));
    let only_undo = History {
        can_undo: true,
        can_redo: false,
        ..History::default()
    };
    reached(block, &["Standup"], only_undo);

    undo(block);
    reached(
        block,
        &["S"],
        History {
            can_undo: true,
            can_redo: true,
            ..History::default()
        },
    );
    undo(block);
    reached(
        block,
        &[],
        History {
            can_undo: false,
            can_redo: true,
            ..History::default()
        },
    );
    redo(block);
    redo(block);
    reached(block, &["Standup"], only_undo);
}
