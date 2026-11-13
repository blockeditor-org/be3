use super::*;
use be_block::{ChildChange, SEARCH_INDEX, be_block_model::BlockRef};
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

fn asked(text: &str) -> SearchAsk {
    SearchAsk {
        text: text.to_owned(),
        within: None,
    }
}

/// Waits until a search for `text` answers exactly `blocks`, every queued edit indexed.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = asked(text);
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

fn note(workspace: Uuid, text: &str) -> Uuid {
    let block = Uuid::new_v4();
    create(
        block,
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
            ChildChange::Add(block)
        ),
        Some(true)
    );
    open(block, TextContent::CONTENT_TYPE);
    type_text(block, 0, text);
    flush();
    block
}

#[test]
fn what_is_typed_is_found_through_the_workspaces_search_index() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
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
    let apples = note(workspace, "apples and pears");
    let bananas = note(workspace, "bananas");
    finds("apples", &[apples]);
    finds("bananas", &[bananas]);
    finds("pea", &[apples]);
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

    replace(
        apples,
        TextContent::CONTENT_TYPE,
        TextBlock::of("cherries only").encode(),
    );
    flush();
    finds("cherries", &[apples]);
    finds("apples", &[]);

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    type_text(bananas, 0, "figs ");
    flush();
    finds("figs", &[bananas]);

    harness.start_server(&address);
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
    retry_now();
    wait_until("reconnected and sent what waited", |shared| {
        shared.connected && shared.waiting == 0
    });
    finds("elderberries", &[elderberries]);
    finds("figs", &[bananas]);
    finds("cherries", &[apples]);
}
