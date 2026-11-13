use super::*;

use be_sdk_api::VersionCommand;

/// The loop keeps going while a version command runs in the lane, so this device can edit a
/// block in the tree between the command reading it and writing it: the edit is merged with
/// what the command writes, never written over.
#[test]
fn an_edit_typed_here_while_a_switch_writes_its_block_is_merged_in() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");
    show_version(versioned.checkout, be_block::CheckoutContent::CONTENT_TYPE);
    hold(versioned.note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });
    version(
        versioned.checkout,
        VersionCommand::CreateBranch {
            name: "draft".into(),
        },
    );
    wait_until("moved onto the new branch", |shared| {
        settled(shared, versioned.checkout)
            && checkout_state(shared, versioned.checkout)
                .is_some_and(|state| state.branch == "draft")
    });
    type_text(versioned.note, 6, "there\n");
    flush_and_send();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    wait_until("saved the change on the branch", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.log.len() == 2 && status.error.is_none())
    });

    let (reached, held) = tokio::sync::oneshot::channel();
    let (go, gone) = tokio::sync::oneshot::channel();
    crate::hold_next_version_write((reached, gone));
    version(
        versioned.checkout,
        VersionCommand::Switch {
            branch: be_block::MAIN_BRANCH.to_owned(),
        },
    );
    assert_eq!(
        held.blocking_recv(),
        Ok(versioned.note),
        "the switch reached writing the note"
    );
    // This device types a line at the top of the note between the switch reading it and
    // writing it; the switch takes the other line away.
    type_text(versioned.note, 0, "Oh\n");
    flush();
    // Sent, though not in step: the switch waits in the lane.
    wait_until("sent the typed line", |shared| {
        shared.waiting == 0 && !shared.draining && !shared.recounting
    });
    go.send(()).expect("the switch waits");

    wait_until("finished the switch with the edit merged in", |shared| {
        settled(shared, versioned.checkout)
            && text_of(shared, versioned.note).as_deref() == Some("Oh\nhello\n")
    });
}
