use super::*;
use be_block::ChildChange;
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
        be_block::BlockMetadata::named("Team"),
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
        be_block::BlockMetadata::named(name),
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
fn a_block_indexed_before_its_type_runs_is_found_by_its_text_once_it_does() {
    let harness = Harness::start();
    harness.connect();
    // The text type's module does not start for the indexer, as one that fails to load.
    with_shared_mut(|shared| shared.search.not_running.insert(TextContent::CONTENT_TYPE));
    let (_, note) = workspace_with_note("Pantry", "apples and pears");

    // Not put by its name alone, which would keep its text out of the index for this commit,
    // and searches say they are not complete while it waits.
    crate::wait_until(|shared| shared.search.waits_for_its_type(note).then_some(()));
    let ask = SearchAsk {
        text: "Pantry".to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        (!answer.complete && answer.hits.is_empty()).then_some(())
    });

    // Once its module runs, as when a text block is opened, the unchanged note is indexed
    // whole.
    with_shared_mut(|shared| shared.search.not_running.remove(&TextContent::CONTENT_TYPE));
    let other = Uuid::new_v4();
    create(
        other,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Elsewhere"),
        Some(TextBlock::of("").encode()),
        false,
    );
    open(other, TextContent::CONTENT_TYPE);
    wait_until("ran the text type", |_| {
        kind_of(TextContent::CONTENT_TYPE).is_some()
    });
    retry_now();
    finds("apples", &[note]);
    finds("Pantry", &[note]);
}
