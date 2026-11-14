use super::*;
use be_block_content::ChildChange;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

#[test]
fn a_search_index_write_that_fails_is_queued_again() {
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
    // The next two writes to the index fail, as a store that cannot seal would.
    with_shared_mut(|shared| shared.search.fail_writes = 2);

    let note = Uuid::new_v4();
    create(
        note,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block_content::BlockMetadata::named("Note"),
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
    type_text(note, 0, "apples and pears");
    flush();

    let ask = SearchAsk {
        text: "apples".to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        (answer.complete && found == [note]).then_some(())
    });
    crate::wait_until(|shared| (shared.search.fail_writes == 0).then_some(()));
    let error = with_shared_mut(|shared| shared.error.clone()).flatten();
    assert!(
        !error.is_some_and(|error| error.starts_with("Indexing for search failed")),
        "a write that fails and then goes through shows no error"
    );
}
