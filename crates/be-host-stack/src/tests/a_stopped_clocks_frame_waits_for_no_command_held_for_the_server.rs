use super::*;

use be_sdk_api::VersionCommand;

/// A version saved while the server has yet to order an edit waits for that order
/// (`after_order`), and a stopped clock's frame does not wait with it (`finish_commands`): with
/// the server silent, every frame would otherwise take the whole budget.
#[test]
fn a_stopped_clocks_frame_waits_for_no_command_held_for_the_server() {
    let harness = Harness::relayed();
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
    wait_until("read the status", |shared| {
        settled(shared, versioned.checkout)
    });

    harness.relay().stall();
    type_text(versioned.note, 5, " there");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    wait_until("held the version for the server's order", |shared| {
        shared.held_for_server > 0
    });
    assert!(
        crate::finish_commands(std::time::Duration::from_secs(10)),
        "a stopped clock's frame waited for a version held for the server"
    );

    harness.relay().cut();
    wait_until("saved the version once reconnected", |shared| {
        shared.connected
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| !status.busy && status.log.len() == 2)
    });
}
