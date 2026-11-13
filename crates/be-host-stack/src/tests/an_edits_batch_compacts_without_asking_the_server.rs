use super::*;

#[test]
fn an_edits_batch_compacts_without_asking_the_server() {
    let harness = Harness::relayed();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 1..16, true);
    let histories = harness
        .relay()
        .count(|method| method == be_block_protocol::__args::BLOCKS_LIST_HISTORY);
    let before = *histories.borrow();
    harness.relay().stall();
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        // The sixteenth fills a tier, so the batch compacts.
        assert_eq!(
            crate::search::write(peer.as_ref(), index, one(&word(16), 17, None)).await,
            Ok(Some(Vec::new())),
            "the batch is written while the server is silent"
        );
    });
    assert_eq!(
        *histories.borrow(),
        before,
        "an edit's batch asked for no history"
    );
    assert!(
        index_of(&device, index).index().segment_count() < 6,
        "the batch compacted"
    );
}
