use super::*;

use be_sdk_api::VersionCommand;

fn head_of(shared: &Shared, repository: Uuid) -> Option<[u8; 32]> {
    version_status(shared, repository)?
        .branches
        .iter()
        .find(|branch| branch.name == be_block_repository::MAIN_BRANCH)
        .map(|branch| branch.head)
}

fn repositories(shared: &Shared) -> Vec<Uuid> {
    shared
        .graph
        .query(crate::Query::Roots)
        .into_iter()
        .filter(|node| node.content_type == be_block_repository::RepositoryContent::CONTENT_TYPE)
        .map(|node| node.id)
        .collect()
}

/// Forking, pulling and pushing run against the device's store offline (guides/blocks/version_control.md, Operations), and what they made reaches the server once the device reconnects.
#[test]
fn a_fork_pulls_and_pushes_offline() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    let versioned = start_versioning("hello\n");
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    version(versioned.repository, VersionCommand::Fork);
    wait_until("forked the repository", |shared| {
        settled(shared, versioned.repository) && repositories(shared).len() == 2
    });
    let fork = with_shared(repositories)
        .unwrap_or_default()
        .into_iter()
        .find(|repository| *repository != versioned.repository)
        .expect("the fork exists");
    show_version(fork, be_block_repository::RepositoryContent::CONTENT_TYPE);
    wait_until("listed the fork's branches", |shared| {
        head_of(shared, fork).is_some()
            && head_of(shared, fork) == head_of(shared, versioned.repository)
    });

    hold(versioned.note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });
    type_text(versioned.note, 5, " there");
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "Upstream".into(),
        },
    );
    wait_until("moved the upstream on", |shared| {
        head_of(shared, versioned.repository) != head_of(shared, fork)
    });
    version(fork, VersionCommand::PullUpstream);
    wait_until("pulled the upstream's branch", |shared| {
        settled(shared, fork) && head_of(shared, fork) == head_of(shared, versioned.repository)
    });

    version(
        fork,
        VersionCommand::NewCheckout {
            branch: be_block_repository::MAIN_BRANCH.to_owned(),
        },
    );
    wait_until("checked the fork out", |shared| {
        settled(shared, fork) && checkouts(shared).len() == 2
    });
    let checkout = with_shared(checkouts)
        .unwrap_or_default()
        .into_iter()
        .find(|checkout| *checkout != versioned.checkout)
        .expect("the fork's checkout exists");
    let note = copy_of(versioned.note, checkout);
    hold(note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the fork's note", |shared| {
        text_of(shared, note).as_deref() == Some("hello there\n")
    });
    type_text(note, 0, "Oh, ");
    version(
        checkout,
        VersionCommand::Finish {
            description: "From the fork".into(),
        },
    );
    wait_until("committed in the fork", |shared| {
        settled(shared, checkout) && head_of(shared, fork) != head_of(shared, versioned.repository)
    });
    version(fork, VersionCommand::PushUpstream);
    wait_until("pushed the fork's branch", |shared| {
        settled(shared, fork)
            && version_status(shared, fork).is_some_and(|status| status.error.is_none())
            && head_of(shared, fork) == head_of(shared, versioned.repository)
    });
    let pushed = with_shared(|shared| head_of(shared, fork)).flatten();

    harness.start_server(&address);
    wait_until("sent what was made offline", |shared| {
        shared.connected && shared.waiting == 0
    });
    let other = Device::online(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    let upstream = other.runtime.block_on(async {
        let repository = peer
            .open::<be_block_repository::RepositoryContent>(versioned.repository)
            .await
            .unwrap()
            .expect("the upstream has content")
            .root();
        let head = repository
            .branch(be_block_repository::MAIN_BRANCH)
            .expect("main has a head");
        peer.load_commit(head).await.unwrap();
        head
    });
    assert_eq!(
        Some(*upstream.hash().as_bytes()),
        pushed,
        "the push reached the server"
    );
}
