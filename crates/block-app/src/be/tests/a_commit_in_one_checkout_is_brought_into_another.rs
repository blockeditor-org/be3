use super::*;

use block_plugin_api::{VersionChangeKind, VersionCommand};

#[test]
fn a_commit_in_one_checkout_is_brought_into_another() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");
    let second = second_checkout(&versioned);
    version_since(
        versioned.checkout,
        be_block::CheckoutContent::CONTENT_TYPE,
        None,
    );
    version_since(second, be_block::CheckoutContent::CONTENT_TYPE, None);

    hold(versioned.note, be_block::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });
    type_text(versioned.note, 5, " there");
    flush();
    wait_until("listed the edit as a change", |shared| {
        version_status(shared, versioned.checkout).is_some_and(|status| {
            status.changes.len() == 1
                && status.changes[0].block_id == versioned.note.into_bytes()
                && status.changes[0].kind == VersionChangeKind::Modified
        })
    });

    version(
        versioned.checkout,
        VersionCommand::Commit {
            message: "Say more".into(),
        },
    );
    wait_until("committed", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout).is_some_and(|status| {
                status.error.is_none() && status.changes.is_empty() && status.log.len() == 2
            })
    });
    wait_until("saw the other checkout fall behind", |shared| {
        version_status(shared, second).is_some_and(|status| status.behind)
    });

    version(second, VersionCommand::Update);
    let note = copy_of(versioned.note, second);
    hold(note, be_block::TextContent::CONTENT_TYPE);
    wait_until("brought the commit in", |shared| {
        text_of(shared, note).as_deref() == Some("hello there\n")
            && version_status(shared, second).is_some_and(|status| {
                !status.behind && status.changes.is_empty() && status.error.is_none()
            })
    });
}
