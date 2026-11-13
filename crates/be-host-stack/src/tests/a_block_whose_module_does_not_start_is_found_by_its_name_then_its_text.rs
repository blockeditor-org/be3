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
fn a_block_whose_module_does_not_start_is_found_by_its_name_then_its_text() {
    let harness = Harness::start();
    harness.connect();
    // The text type's module does not start for the indexer, as one that fails to load.
    with_shared_mut(|shared| shared.search.not_running.insert(TextContent::CONTENT_TYPE));
    let (workspace, note) = workspace_with_note("Pantry", "apples and pears");

    // Put by its name, marked partial, and searches are complete.
    finds("Pantry", &[note]);
    finds("apples", &[]);

    // Once its module runs, as when a text block is opened, the note queued again is put in
    // full at the same commit, which replaces the partial entry.
    with_shared_mut(|shared| shared.search.not_running.remove(&TextContent::CONTENT_TYPE));
    let other = Uuid::new_v4();
    create(
        other,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Elsewhere"),
        Some(TextBlock::of("").encode()),
        false,
    );
    open(other, TextContent::CONTENT_TYPE);
    wait_until("ran the text type", |_| {
        kind_of(TextContent::CONTENT_TYPE).is_some()
    });
    // The mirror lists the workspace's children, so the note can be placed once the
    // catch-up that named it is done with it.
    want(Owner::Host, vec![Query::Children(workspace)]);
    crate::wait_until(|shared| shared.graph.get(note).map(|_| ()));
    with_shared_mut(|shared| shared.search.reread([note]));
    retry_now();
    finds("apples", &[note]);
    finds("Pantry", &[note]);
}
