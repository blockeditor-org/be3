use super::*;
use be_block_content::{SEARCH_INDEX, be_block_model::BlockRef};
use be_type_text::TextContent;

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
fn a_device_that_never_wrote_an_index_hears_what_another_device_indexes() {
    let harness = Harness::start();
    let other = Device::online(&harness);
    let (workspace, index) = other.index_workspace();
    let apples = other.index_note(workspace, index, "apples");

    // This device only reads the index: it never wrote it, so it keeps no head of its own.
    harness.connect();
    // As the files pane does: the roots are what a search reads the indexes of.
    crate::want(Owner::Host, vec![crate::graph::Query::Roots]);
    wait_until("listed the workspace", |shared| {
        shared.graph.get(workspace).is_some()
    });
    wait_until("knew the workspace's index", |shared| {
        shared
            .graph
            .get(workspace)
            .and_then(|node| node.metadata.get(&SEARCH_INDEX))
            == Some(BlockRef(index))
    });
    // Snippets are cut by the note's type, which this device loads as it opens one.
    open(apples, TextContent::CONTENT_TYPE);
    wait_until("knew the note's type", |_| {
        is_known(TextContent::CONTENT_TYPE)
    });
    finds("apples", &[apples]);

    // Opening the note indexed it here too, so the index moved on from the head the other
    // device knows: a device new to it writes the next batch, from the server's head.
    let another = Device::online(&harness);
    let bananas = another.index_note(workspace, index, "bananas");
    finds("bananas", &[bananas]);
    finds("apples", &[apples]);
}
