use super::*;

use be_sdk_api::VersionChangeKind;

#[test]
fn a_reconnected_session_hears_the_edits_of_what_it_versions() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let versioned = start_versioning("hello\n");
    show_version(versioned.checkout, be_block::CheckoutContent::CONTENT_TYPE);
    wait_until("read the checkout's versions", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.error.is_none() && status.changes.is_empty())
    });
    flush();

    // The note's own listing, which is how the new connection hears its heads again: the
    // graph's relist after a reconnect lists much else besides.
    let note = versioned.note;
    let mut listings = harness.relay().count(move |method, args| {
        method == be_block_protocol::__args::BLOCKS_LIST_BLOCKS
            && be_wasm_ops::decode::<be_block_protocol::__args::BlocksListBlocksArgs>(args)
                .is_some_and(|asked| asked.listing == be_block_protocol::Listing::Block(note))
    });
    harness.relay().stall();
    harness.miss_a_ping();
    retry_now();
    wait_until("connected again", |shared| shared.connected);
    runtime()
        .block_on(listings.wait_for(|listings| *listings > 0))
        .expect("the new connection lists what the session versions");

    // Another device edits the note in the checkout, which the new connection must hear.
    let device = Device::online(&harness);
    device.runtime.block_on(async {
        let peer = std::sync::Arc::clone(&device.peer.borrow());
        let head = peer.summary(note).await.expect("the note is there").head;
        peer.save(note, &be_type_text::TextBlock::of("hello there\n"), head)
            .await
            .expect("the other device edits the note");
    });
    wait_until("listed the other device's edit as a change", |shared| {
        version_status(shared, versioned.checkout).is_some_and(|status| {
            status
                .changes
                .iter()
                .any(|change| change.kind == VersionChangeKind::Modified)
        })
    });
}
