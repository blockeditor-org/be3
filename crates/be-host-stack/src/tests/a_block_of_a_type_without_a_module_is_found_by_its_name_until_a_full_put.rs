use super::*;
use be_block_content::ChildChange;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// Waits until a search for `text` answers exactly `blocks`, every queued edit indexed.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        (answer.complete && found == blocks).then_some(())
    });
}

/// A workspace with a text note in it, made with `text` and named `name`, not opened: the
/// workspace and the note.
fn workspace_with_note(name: &str, text: &str) -> (Uuid, Uuid) {
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Team"),
        None,
        false,
    );
    wait_until("knew the folder's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let note = Uuid::new_v4();
    create(
        note,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block_content::BlockMetadata::named(name),
        Some(TextBlock::of(text).encode()),
        false,
    );
    assert_eq!(
        change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(note)
        ),
        Some(true)
    );
    flush();
    (workspace, note)
}

#[test]
fn a_block_of_a_type_without_a_module_is_found_by_its_name_until_a_full_put() {
    let harness = Harness::start();
    harness.connect();
    // This device has no module for the text type, as for an app not installed here.
    with_shared_mut(|shared| shared.search.no_module.insert(TextContent::CONTENT_TYPE));
    let (_, note) = workspace_with_note("Pantry", "apples and pears");

    // Found by its name, which is all this device can read of it, and searches are complete.
    finds("Pantry", &[note]);
    finds("apples", &[]);

    // A device with the module puts the same commit in full, as this one does once it has the
    // module: the full put replaces the partial one, though the note did not change.
    with_shared_mut(|shared| {
        shared.search.no_module.remove(&TextContent::CONTENT_TYPE);
        shared.search.reread([note]);
    });
    retry_now();
    finds("apples", &[note]);
    finds("Pantry", &[note]);
}
