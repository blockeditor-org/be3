use super::*;
use be_block::{SEARCH_INDEX, be_block_model::BlockRef};
use be_block_client::Peer;
use be_block_store::MemoryStore;
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

/// Makes a note under `workspace` and indexes it, as the device that edits it does.
async fn note(peer: &Peer<MemoryStore>, workspace: Uuid, index: Uuid, text: &str) -> Uuid {
    let block = Uuid::new_v4();
    peer.create_with_id::<TextContent>(block, be_block_graph::BlockParent::Block(workspace))
        .await
        .expect("the other device makes a note");
    let saved = peer
        .save(block, &TextBlock::of(text), None)
        .await
        .expect("the other device saves it");
    let head = saved.published().expect("the note is published");
    let commit = peer.load_commit(head).await.expect("its commit loads");
    let batch = be_lib_search::Batch {
        put: vec![be_lib_search::Put {
            doc: be_lib_search::DocId {
                block: block.as_bytes().to_vec(),
                key: String::new(),
            },
            commit: be_lib_search::Commit {
                time: commit.time,
                id: *head.hash().as_bytes(),
            },
            source: None,
            text: Some(be_lib_search::Text {
                name: "Note".to_owned(),
                body: text.to_owned(),
            }),
        }],
        ..be_lib_search::Batch::default()
    };
    assert_eq!(
        crate::search::write(peer, index, batch).await,
        Ok(true),
        "the other device indexes its note"
    );
    peer.drain(|_| false)
        .await
        .expect("the other device publishes its batch");
    block
}

#[test]
fn a_device_that_never_wrote_an_index_hears_what_another_device_indexes() {
    let harness = Harness::start();
    let other = Device::online(&harness);
    let workspace = Uuid::new_v4();
    let index = Uuid::new_v4();
    let apples = other.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        peer.create_with_id::<WorkspaceContent>(workspace, be_block_graph::BlockParent::Root)
            .await
            .expect("the other device makes a workspace");
        crate::search::make_index(peer.as_ref(), workspace, index)
            .await
            .expect("the other device makes its index");
        peer.define(workspace, |metadata| {
            metadata.insert(&SEARCH_INDEX, &BlockRef(index));
        })
        .await
        .expect("the other device names the index");
        peer.drain(|_| false)
            .await
            .expect("the other device publishes the index");
        note(&peer, workspace, index, "apples").await
    });

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
    let bananas = another.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&another.peer.borrow());
        note(&peer, workspace, index, "bananas").await
    });
    finds("bananas", &[bananas]);
    finds("apples", &[apples]);
}
