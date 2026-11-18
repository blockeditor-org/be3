use super::*;
use be_block_content::ChildChange;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// A local account's search indexes what it writes on the device (guides/blocks/client.md, Saving and the outbox),
/// with no server to catch up from.
#[test]
fn a_local_account_finds_the_text_it_wrote() {
    let harness = Harness::start();
    start(Config {
        local: true,
        server_url: String::new(),
        organization: Uuid::now_v7(),
        ..harness.config(harness.directory.join("local"))
    });
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Orchard"),
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
        be_block_content::BlockMetadata::named("Notes"),
        Some(TextBlock::of("Quinces ripen late in the autumn").encode()),
        false,
    );
    assert!(made(change_child(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        ChildChange::Add(note)
    )));
    flush();
    let ask = SearchAsk {
        text: "ripen".to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until("found the text it wrote", |shared| {
        let answer = shared.search.answer(&ask)?;
        let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        (answer.complete && found == [note]).then_some(())
    });
    let snippet = with_shared(|shared| shared.search.answer(&ask))
        .flatten()
        .and_then(|answer| answer.hits.first().map(|hit| hit.snippet.clone()));
    assert!(
        snippet
            .as_deref()
            .is_some_and(|snippet| snippet.contains("ripen late")),
        "the hit shows the text it matched: {snippet:?}"
    );
    // Indexing settles: what it writes of the index is not queued to be indexed again.
    wait_until("indexed everything", |shared| shared.search_busy.is_none());
}
