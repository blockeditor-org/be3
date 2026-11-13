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
fn a_block_renamed_in_the_files_is_found_by_its_new_name() {
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
    // The files list the workspace's children, as the files pane shows the note to rename.
    want(Owner::Host, vec![Query::Children(workspace)]);
    crate::wait_until(|shared| shared.graph.get(note).map(|_| ()));
    finds("Pantry", &[note]);

    // Renamed in the files, never opened, and without reconnecting: found by its new name.
    set_name(note, Some("Larder".to_owned()));
    flush();
    finds("Larder", &[note]);
    finds("Pantry", &[]);
    finds("apples", &[note]);
}
