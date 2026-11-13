use super::*;
use be_block::SEARCH_INDEX;
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

fn workspace(name: &str) -> Uuid {
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named(name),
        None,
        false,
    );
    workspace
}

#[test]
fn the_host_catches_up_what_other_devices_wrote_and_masks_what_left_the_tree() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    let team = workspace("Team");
    wait_until("knew the folder's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let other_space = workspace("Other");
    wait_until("named both indexes", |shared| {
        [team, other_space].iter().all(|space| {
            shared
                .graph
                .get(*space)
                .is_some_and(|node| node.metadata.get(&SEARCH_INDEX).is_some())
        })
    });

    // The workspaces and their indexes reach the server before it goes, so the other device
    // can write under them.
    flush_and_send();

    // While the host is away, another device writes a note: no indexer saw it, so only the
    // catch-up on reconnecting can.
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    harness.start_server(&address);
    let other = Device::online(&harness);
    let quinces = Uuid::new_v4();
    other.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        peer.create_with_id::<TextContent>(quinces, be_block_graph::BlockParent::Block(team))
            .await
            .expect("the other device makes a note");
        peer.save(quinces, &TextBlock::of("quinces in the cellar"), None)
            .await
            .expect("the other device saves it");
        peer.drain(|_| false)
            .await
            .expect("the other device publishes it");
    });
    // The host may have come back before the note was published; coming back once more, it
    // catches up from where it was.
    harness.stop_server();
    wait_until("noticed the server was gone again", |shared| {
        !shared.connected
    });
    harness.start_server(&address);
    retry_now();
    wait_until("reconnected", |shared| shared.connected);
    finds("quinces", None, &[quinces]);
    finds("quinces", Some(team), &[quinces]);

    // The note moves to the other workspace: a search of the first no longer lists it.
    set_parent(quinces, be_block_graph::BlockParent::Block(other_space));
    wait_until("moved the note", |shared| {
        shared.graph.get(quinces).is_some_and(|node| {
            node.parent == Some(be_block_graph::BlockParent::Block(other_space))
        })
    });
    finds("quinces", Some(team), &[]);

    // Coming back once more, the host sees the note has left and masks it in the first
    // workspace's index, which another device reads from the server: nothing of it is left.
    harness.stop_server();
    wait_until("noticed the server was gone a third time", |shared| {
        !shared.connected
    });
    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and sent what waited", |shared| {
        shared.connected && shared.waiting == 0 && shared.search.indexed()
    });
    let index = node(team)
        .and_then(|node| node.metadata.get(&SEARCH_INDEX))
        .map(|be_block::be_block_model::BlockRef(index)| index)
        .expect("the workspace names its index");
    let hits = other.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        crate::search::search_index(
            peer.as_ref(),
            index,
            None,
            &be_lib_search::Query::parse("quinces"),
            10,
        )
        .await
        .expect("the other device searches the index")
    });
    assert!(
        hits.is_empty(),
        "the index still lists a block that left: {hits:?}"
    );
}
