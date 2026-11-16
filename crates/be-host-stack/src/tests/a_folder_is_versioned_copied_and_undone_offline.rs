use super::*;

use be_sdk_api::VersionCommand;

fn saved(shared: &Shared, block: Uuid) -> Option<usize> {
    version_status(shared, block).map(|status| status.log.len())
}

/// Starting to keep versions, a second checkout and undo run against the device's store
/// offline (guides/blocks/version_control.md, Operations), and what they made reaches the server once the
/// device reconnects.
#[test]
fn a_folder_is_versioned_copied_and_undone_offline() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    wait_until("loaded the graph", |shared| {
        shared.graph.ready(super::graph::Query::Roots)
    });
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);

    let versioned = start_versioning("hello\n");
    let second = second_checkout(&versioned);
    show_version(
        versioned.checkout,
        be_block_repository::CheckoutContent::CONTENT_TYPE,
    );
    hold(versioned.note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });
    type_text(versioned.note, 5, " there");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    wait_until("saved the change", |shared| {
        settled(shared, versioned.checkout) && saved(shared, versioned.checkout) == Some(2)
    });
    let finished = with_shared(|shared| {
        version_status(shared, versioned.checkout)?
            .operations
            .iter()
            .find(|operation| operation.description.starts_with("Saved the change"))
            .map(|operation| operation.id)
    })
    .flatten()
    .expect("saving was recorded");
    version(
        versioned.checkout,
        VersionCommand::Undo {
            operation: finished,
        },
    );
    wait_until("undid saving the change", |shared| {
        settled(shared, versioned.checkout)
            && saved(shared, versioned.checkout) == Some(1)
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.error.is_none())
    });
    assert!(!with_shared(|shared| shared.connected).unwrap_or(true));

    harness.start_server(&address);
    wait_until("sent what was made offline", |shared| {
        shared.connected && shared.waiting == 0
    });
    let other = Device::online(&harness);
    assert_eq!(other.read(copy_of(versioned.note, second)), "hello\n");
    assert_eq!(other.read(versioned.note), "hello there\n");
}
