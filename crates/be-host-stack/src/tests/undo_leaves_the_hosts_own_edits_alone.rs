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

fn by_editor(editor: &mut Editor<ChecklistContent>, edit: &Edit) {
    editor.send(be_sdk_type_guest::editor::FromEditor::Operate {
        gesture: None,
        operation: ChecklistContent::encode_operation(edit),
    });
}

/// The host's own edits (`operate`, as `HOST_EDITOR`) are no one's to undo: a block they alone
/// changed offers no undo, and undoing a person's edit made after one takes back that edit
/// only, never the host's with it.
#[test]
fn undo_leaves_the_hosts_own_edits_alone() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, ChecklistContent::CONTENT_TYPE);
    let mut editor = Editor::<ChecklistContent>::attach(block);

    let (_, by_host) = Checklist::add("Set up");
    operate(block, ChecklistContent::encode_operation(&by_host));
    reached(block, &["Set up"], History::default());

    let (_, by_person) = Checklist::add("Standup");
    by_editor(&mut editor, &by_person);
    reached(
        block,
        &["Set up", "Standup"],
        History {
            can_undo: true,
            can_redo: false,
            ..History::default()
        },
    );

    undo(block);
    reached(
        block,
        &["Set up"],
        History {
            can_undo: false,
            can_redo: true,
            ..History::default()
        },
    );
}
