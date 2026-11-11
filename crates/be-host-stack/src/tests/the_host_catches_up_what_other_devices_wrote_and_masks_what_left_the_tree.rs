use super::*;
use be_block::{ChildChange, SEARCH_INDEX, be_block_model::BlockRef};
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

fn asked(text: &str, within: Option<Uuid>) -> SearchAsk {
    SearchAsk {
        text: text.to_owned(),
        within,
    }
}

/// Waits until a search for `text` answers exactly `blocks`, every queued edit indexed.
fn finds(text: &str, within: Option<Uuid>, blocks: &[Uuid]) {
    let ask = asked(text, within);
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
fn the_host_catches_up_what_other_devices_wrote_and_masks_what_left_the_tree() {
    let mut harness = Harness::start();
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
    wait_until("named the index", |_| {
        node(workspace).is_some_and(|node| node.metadata.get(&SEARCH_INDEX).is_some())
    });

    // Another device writes a note with no indexer: nothing indexes it but the catch-up.
    let other = Device::online(&harness);
    let quinces = Uuid::new_v4();
    other.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        peer.create_with_id::<TextContent>(quinces, be_block_graph::BlockParent::Block(workspace))
            .await
            .expect("the other device makes a note");
        peer.save(quinces, &TextBlock::of("quinces in the cellar"), None)
            .await
            .expect("the other device saves it");
        peer.drain(|_| false)
            .await
            .expect("the other device publishes it");
    });
    let _ = ChildChange::Add(quinces);
    let _ = BlockRef(Uuid::nil());
    finds("quinces", None, &[quinces]);

    // It leaves the tree for another workspace: searches of the first no longer list it, and
    // the index drops it for good once the catch-up has seen it go.
    let elsewhere = Uuid::new_v4();
    create(
        elsewhere,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Other"),
        None,
        false,
    );
    other.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        peer.set_parent(quinces, be_block_graph::BlockParent::Block(elsewhere))
            .await
            .expect("the other device moves it");
        peer.drain(|_| false)
            .await
            .expect("the other device publishes the move");
    });
    finds("quinces", Some(workspace), &[]);
}
