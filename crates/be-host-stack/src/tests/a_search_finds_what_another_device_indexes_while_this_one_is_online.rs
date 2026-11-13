use super::*;
use be_block::{ChildChange, SEARCH_INDEX, be_block_model::BlockRef};
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// Waits until a search for `text` answers exactly `blocks`.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    let mut expected = blocks.to_vec();
    expected.sort();
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        let mut found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        found.sort();
        (answer.complete && found == expected).then_some(())
    });
}

#[test]
fn a_search_finds_what_another_device_indexes_while_this_one_is_online() {
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
    let apples = Uuid::new_v4();
    create(
        apples,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block::BlockMetadata::named("Note"),
        Some(TextBlock::of("").encode()),
        false,
    );
    assert_eq!(
        change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(apples)
        ),
        Some(true)
    );
    open(apples, TextContent::CONTENT_TYPE);
    type_text(apples, 0, "apples");
    flush();
    finds("apples", &[apples]);
    // The mirror takes the index's name once its root's metadata is settled, which a search
    // over the index does not wait for.
    let index = crate::wait_until(|shared| {
        shared
            .graph
            .get(workspace)?
            .metadata
            .get(&SEARCH_INDEX)
            .map(|BlockRef(index)| index)
    });
    // This device's batches are on the server: what the other device writes next lands on top
    // of them while this one stays connected, so only what the server pushes tells it.
    wait_until("sent what waited", |shared| {
        shared.connected && shared.waiting == 0
    });
    finds("elderberries", &[]);

    let other = Device::online(&harness);
    let elderberries = other.index_note(workspace, index, "elderberries");
    finds("elderberries", &[elderberries]);
    finds("apples", &[apples]);
}
