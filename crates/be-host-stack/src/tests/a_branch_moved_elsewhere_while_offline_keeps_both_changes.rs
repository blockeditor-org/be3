use super::*;

use be_block_commit::CommitKind;
use be_block_repository::{MAIN_BRANCH, Repository, RepositoryContent};
use be_sdk_api::VersionCommand;

/// A branch another device moved while this one saved a change on it offline diverges, as a
/// block's head does: the merge keeps the other device's commit at the branch and this one's
/// beside it, so neither is lost.
#[test]
fn a_branch_moved_elsewhere_while_offline_keeps_both_changes() {
    // On the manual clock the host only tries the server again at `retry_now`, so the other
    // device moves the branch first.
    let mut harness = Harness::start().on_manual_clock();
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
    wait_until("read the status", |shared| {
        settled(shared, versioned.checkout)
    });
    let other = Device::online(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    let repository = |peer: &be_block_client::Peer<be_block_store::MemoryStore>| {
        other.runtime.block_on(async {
            let head = peer.remote_head(versioned.repository).await.unwrap();
            let content = peer
                .open::<RepositoryContent>(versioned.repository)
                .await
                .unwrap()
                .expect("the repository has content")
                .root();
            (content, head)
        })
    };
    let (before, _) = repository(&peer);
    let started = before.branch(MAIN_BRANCH).expect("main has a head");

    harness.stop_server();
    wait_until("tried the server and found it gone", |shared| {
        !shared.connected && shared.error.is_some()
    });
    type_text(versioned.note, 5, " there");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    wait_until("saved the change offline", |shared| {
        version_status(shared, versioned.checkout)
            .is_some_and(|status| !status.busy && status.log.len() == 2)
    });

    harness.start_server(&address);
    other.reconnect(&harness);
    let (held, head) = repository(&peer);
    // Any commit the other device holds stands in for its snapshot: the merge reads only the
    // branches.
    let elsewhere = head.expect("the repository has a head");
    let moved: Repository = held.with_branch(MAIN_BRANCH, Some(elsewhere));
    other
        .runtime
        .block_on(peer.save(versioned.repository, &RepositoryContent::new(&moved), head))
        .unwrap()
        .published()
        .expect("the other device moved main");
    retry_now();
    wait_until("sent what was saved offline", |shared| {
        shared.connected && shared.waiting == 0
    });

    let (merged, _) = repository(&peer);
    assert_eq!(merged.branch(MAIN_BRANCH), Some(elsewhere));
    let kept = merged
        .branch("main-2")
        .expect("this device's change was kept beside main");
    assert_ne!(kept, started);
    let kind = other.runtime.block_on(peer.load_reached(kept)).unwrap().kind;
    assert_eq!(kind, CommitKind::Snapshot("There".into()));
}
