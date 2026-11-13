use super::*;

/// The documents a search for `word` finds in `index`, at the head `device` reads.
fn found(device: &Device, index: Uuid, word: &str) -> usize {
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device
        .runtime
        .block_on(crate::search::search_index(
            peer.as_ref(),
            index,
            None,
            &be_lib_search::Query::parse(word),
            10,
        ))
        .expect("the index reads")
        .len()
}

#[test]
fn a_batch_on_an_index_head_the_server_let_go_is_written_on_the_newer_one() {
    let harness = Harness::start();
    let first = Device::online(&harness);
    let (workspace, index) = first.index_workspace();
    first.index_note(workspace, index, "apples");

    // The second device hears of the index's head but reads nothing of it yet.
    let second = Device::online(&harness);
    let peer = std::sync::Arc::clone(&second.peer.borrow());
    let heard = second
        .runtime
        .block_on(peer.summary(index))
        .expect("the second device reads the index's summary")
        .head
        .expect("the index has a head");
    assert_eq!(peer.known_remote_head(index), Some(heard));

    // The first device's next batch lands in the same slot of time: the server keeps one state
    // of an index a slot (`be_server::retention`), so it lets go of the head the second device
    // heard of, with the parts only that head held. (Two batches either side of a slot's end
    // keep it, and the second device's batch then meets the first's as a diverged block.)
    first.index_note(workspace, index, "bananas");

    // The batch goes on the server's newer head rather than failing on the one let go of.
    second.index_note(workspace, index, "cherries");
    assert_ne!(peer.known_remote_head(index), Some(heard));
    for word in ["apples", "bananas", "cherries"] {
        assert_eq!(found(&second, index, word), 1, "{word} is in the index");
    }
}
