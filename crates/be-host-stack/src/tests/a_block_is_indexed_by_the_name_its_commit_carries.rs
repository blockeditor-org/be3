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

/// A rename is a commit of its own, which the indexer is handed before the mirror reads the
/// block's new name: the index takes the name from the commit, since it keeps one entry for a
/// commit and a later put of the same commit, from a device that read the name right, changes
/// nothing.
#[test]
fn a_block_is_indexed_by_the_name_its_commit_carries() {
    let harness = Harness::start();
    harness.connect();
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
        be_block_content::BlockMetadata::named("Packing list"),
        Some(TextBlock::of("").encode()),
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
    open(note, TextContent::CONTENT_TYPE);
    type_text(note, 0, "socks");
    flush();
    finds("socks", &[note]);
    finds("packing", &[note]);

    // The mirror has not read the name the next commit carries.
    with_shared_mut(|shared| shared.graph.forget_name(note));
    type_text(note, 0, "boots ");
    flush();
    finds("boots", &[note]);
    finds("packing", &[note]);
}
