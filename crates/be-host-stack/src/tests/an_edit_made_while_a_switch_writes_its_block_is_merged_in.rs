use super::*;

use be_sdk_api::VersionCommand;

/// Version commands run beside the loop, so a block in the tree can change between a command
/// reading it and writing it: the change is merged with what the command writes, never written
/// over.
#[test]
fn an_edit_made_while_a_switch_writes_its_block_is_merged_in() {
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

    let (reached, held) = std::sync::mpsc::channel();
    let (go, gone) = std::sync::mpsc::channel();
    *crate::version::BEFORE_WRITE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((reached, gone));
    version(
        versioned.checkout,
        VersionCommand::Switch {
            branch: be_block::MAIN_BRANCH.to_owned(),
        },
    );
    assert_eq!(
        held.recv_timeout(Duration::from_secs(300)),
        Ok(versioned.note),
        "the switch reached writing the note"
    );
    // Someone adds a line to the note between the switch reading it and writing it; the switch
    // takes the other line away.
    let runtime = runtime();
    runtime.block_on(async {
        let peer = harness.outside_peer().await;
        let head = peer
            .remote_head(versioned.note)
            .await
            .expect("the note's head is read");
        peer.save(
            versioned.note,
            &be_type_text::TextBlock::of("Oh\nhello\nthere\n"),
            head,
        )
        .await
        .expect("the edit is saved");
    });
    go.send(()).expect("the switch waits");

    wait_until("finished the switch with the edit merged in", |shared| {
        settled(shared, versioned.checkout)
            && text_of(shared, versioned.note).as_deref() == Some("Oh\nhello\n")
    });
}
