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
    let index = node(workspace)
        .and_then(|node| node.metadata.get(&SEARCH_INDEX))
        .map(|BlockRef(index)| index)
        .expect("the workspace names its index");
    // This device's batches are on the server: what the other device writes next lands on top
    // of them while this one stays connected, so only what the server pushes tells it.
    wait_until("sent what waited", |shared| {
        shared.connected && shared.waiting == 0
    });
    finds("elderberries", &[]);

    let other = Device::online(&harness);
    let elderberries = Uuid::new_v4();
    other.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        peer.create_with_id::<TextContent>(
            elderberries,
            be_block_graph::BlockParent::Block(workspace),
        )
        .await
        .expect("the other device makes a note");
        let saved = peer
            .save(elderberries, &TextBlock::of("elderberries"), None)
            .await
            .expect("the other device saves it");
        let head = saved.published().expect("the note is published");
        let commit = peer.load_commit(head).await.expect("its commit loads");
        let batch = be_lib_search::Batch {
            put: vec![be_lib_search::Put {
                doc: be_lib_search::DocId {
                    block: elderberries.as_bytes().to_vec(),
                    key: String::new(),
                },
                commit: be_lib_search::Commit {
                    time: commit.time,
                    id: *head.hash().as_bytes(),
                },
                source: None,
                text: Some(be_lib_search::Text {
                    name: "Note".to_owned(),
                    body: "elderberries".to_owned(),
                }),
            }],
            ..be_lib_search::Batch::default()
        };
        assert_eq!(
            crate::search::write(peer.as_ref(), index, batch).await,
            Ok(true),
            "the other device indexes its note"
        );
        peer.drain(|_| false)
            .await
            .expect("the other device publishes its batch");
    });
    finds("elderberries", &[elderberries]);
    finds("apples", &[apples]);
}
