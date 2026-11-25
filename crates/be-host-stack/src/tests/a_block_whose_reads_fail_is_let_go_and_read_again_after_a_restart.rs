use super::*;
use be_block_content::{SEARCH_INDEX, be_block_model::BlockRef};
use be_type_text::{TextBlock, TextContent};

use crate::search::{FAILING_READS, SearchAsk};

/// How often the indexer has tried to read `block`.
fn tries(block: Uuid) -> u32 {
    FAILING_READS
        .lock()
        .unwrap()
        .get(&block)
        .map_or(0, |(_, tries)| *tries)
}

fn wait_for_tries(block: Uuid, at_least: u32) {
    crate::wait_until(&format!("tried to read the block {at_least} times"), |_| {
        (tries(block) >= at_least).then_some(())
    });
}

/// Waits until a search for `text` answers exactly `blocks`, with nothing left to index.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(
        &format!("found {text:?} in what it should, with nothing left to send"),
        |shared| {
            let answer = shared.search.answer(&ask)?;
            let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
            (answer.complete && found == blocks && shared.waiting == 0 && !shared.draining)
                .then_some(())
        },
    );
}

fn reconnect(harness: &mut Harness) {
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    harness.start_server();
    retry_now();
    wait_until("reconnected", |shared| shared.connected);
}

#[test]
fn a_block_whose_reads_fail_is_let_go_and_read_again_after_a_restart() {
    let mut harness = Harness::start();
    let other = Device::online(&harness);
    let (workspace, index) = other.index_workspace();
    // A note the other device does not index, so only a catch-up here finds it.
    let note = {
        let peer = std::sync::Arc::clone(&other.peer.borrow());
        other.runtime.block_on(async {
            let note = peer
                .create::<TextContent>(be_block_graph::BlockParent::Block(workspace))
                .await
                .expect("the other device makes a note");
            peer.save(note, &TextBlock::of("apples"), None)
                .await
                .expect("the other device saves it");
            peer.drain(|_| false)
                .await
                .expect("the other device publishes it");
            note
        })
    };
    FAILING_READS.lock().unwrap().insert(note, (true, 0));

    harness.connect();
    crate::want(Owner::Host, vec![crate::graph::Query::Roots]);
    wait_until("knew the workspace's index", |shared| {
        shared
            .graph
            .get(workspace)
            .and_then(|node| node.metadata.get(&SEARCH_INDEX))
            == Some(BlockRef(index))
    });
    // Each connection's catch-up reads the note once and holds the watermark back, until the
    // third lets it go.
    let settled =
        || with_shared_mut(|shared| shared.search.settled_at.get(&index).copied()).flatten();
    wait_for_tries(note, 1);
    finds("apples", &[]);
    assert_eq!(settled(), None, "a block not read holds the watermark back");
    reconnect(&mut harness);
    wait_for_tries(note, 2);
    finds("apples", &[]);
    assert_eq!(settled(), None);
    reconnect(&mut harness);
    wait_for_tries(note, 3);
    crate::wait_until("settled the search index", |shared| {
        shared.search.settled_at.get(&index).copied()
    });

    // Its catch-ups now start past it, but a restart still reads it again: what was let go is
    // kept on this device. Read again, it is in no catch-up's page, so the watermark still
    // moves.
    stop();
    harness.connect();
    wait_for_tries(note, 4);
    crate::wait_until("settled the search index", |shared| {
        shared.search.settled_at.get(&index).copied()
    });
    finds("apples", &[]);

    // Once its commit reads, the next connection's catch-up indexes it.
    FAILING_READS.lock().unwrap().insert(note, (false, 4));
    reconnect(&mut harness);
    finds("apples", &[note]);
    FAILING_READS.lock().unwrap().remove(&note);
}
