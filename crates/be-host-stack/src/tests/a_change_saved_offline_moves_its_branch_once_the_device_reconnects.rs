use super::*;

use be_block_commit::CommitKind;
use be_sdk_api::VersionCommand;

/// Version control runs against the device's store offline: a change saved there waits in the
/// outbox with the repository's move of its branch and the hold of its snapshot, and another
/// device finds both once it is sent.
#[test]
fn a_change_saved_offline_moves_its_branch_once_the_device_reconnects() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    let versioned = start_versioning("hello\n");
    show_version(
        versioned.checkout,
        be_block_repository::CheckoutContent::CONTENT_TYPE,
    );
    hold(versioned.note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });
    wait_until("read the status", |shared| settled(shared, versioned.checkout));

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    type_text(versioned.note, 5, " there");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    let saved = crate::wait_until("saved the change or failed to", |shared| {
        version_status(shared, versioned.checkout)
            .filter(|status| !status.busy && (status.log.len() == 2 || status.error.is_some()))
            .map(|status| (status.log.len(), status.error.clone()))
    });
    assert_eq!(saved, (2, None), "the change was not saved offline");
    assert!(status().waiting > 0, "the change did not wait to be sent");

    harness.start_server(&address);
    wait_until("sent what was saved offline", |shared| {
        shared.connected && shared.waiting == 0
    });
    let other = Device::online(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    let description = other.runtime.block_on(async {
        let repository = peer
            .open::<be_block_repository::RepositoryContent>(versioned.repository)
            .await
            .unwrap()
            .expect("the repository has content")
            .root();
        let head = repository
            .branch(be_block_repository::MAIN_BRANCH)
            .expect("main has a head");
        peer.load_commit(head).await.unwrap().kind
    });
    assert_eq!(description, CommitKind::Snapshot("There".into()));
}
