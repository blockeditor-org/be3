use super::*;

use be_sdk_api::VersionCommand;

#[test]
fn a_status_after_a_version_command_reads_nothing_the_command_read() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");
    show_version(
        versioned.checkout,
        be_block_repository::CheckoutContent::CONTENT_TYPE,
    );
    hold(versioned.note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });
    type_text(versioned.note, 5, " there");
    flush_and_send();
    wait_until("listed the edit as a change", |shared| {
        version_status(shared, versioned.checkout).is_some_and(|status| status.changes.len() == 1)
    });

    // The command reads the working commit and tree the status read, and the status after it
    // reads the commits and trees the command wrote: each is read from the peer once.
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "Say more".into(),
        },
    );
    wait_until("committed, and read the status again", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout).is_some_and(|status| {
                status.error.is_none() && status.changes.is_empty() && status.log.len() == 2
            })
    });
    let (reads, read_again) = crate::version::reads();
    assert!(reads > 0, "the status read its trees and commits");
    assert_eq!(read_again, Vec::new(), "nothing was read twice");
}
