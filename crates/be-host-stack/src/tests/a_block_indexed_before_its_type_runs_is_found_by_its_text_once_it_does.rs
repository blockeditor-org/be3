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

#[test]
fn a_block_indexed_before_its_type_runs_is_found_by_its_text_once_it_does() {
    let harness = Harness::start();
    harness.connect();
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
    // The indexer takes the text type's module as not running, as on a device that has not
    // started it yet: nothing it could start reads the note's text.
    with_shared_mut(|shared| shared.search.not_running.insert(TextContent::CONTENT_TYPE));

    let note = Uuid::new_v4();
    create(
        note,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block::BlockMetadata::named("Pantry"),
        Some(TextBlock::of("apples and pears").encode()),
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
    // Not put by its name alone: a later put of the same commit with its text would be
    // skipped as no newer.
    crate::wait_until(|shared| shared.search.waits_for_its_type(note).then_some(()));
    finds("Pantry", &[]);

    // Once its module runs the note is indexed whole, though it did not change.
    with_shared_mut(|shared| shared.search.not_running.remove(&TextContent::CONTENT_TYPE));
    // Opening another text block starts the module.
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
