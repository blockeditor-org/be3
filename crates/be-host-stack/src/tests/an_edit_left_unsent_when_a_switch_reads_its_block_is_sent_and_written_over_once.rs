use super::*;

use be_sdk_api::VersionCommand;

/// A version command that read this device's edit of a block before it was sent waits for it
/// to be sent and writes over it: the edit is in what it writes once, never merged in again.
#[test]
fn an_edit_left_unsent_when_a_switch_reads_its_block_is_sent_and_written_over_once() {
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

    // This device types a line at the top of the note and seals it, but the outbox holds it.
    crate::worker::HOLD_DRAINS.store(true, std::sync::atomic::Ordering::SeqCst);
    type_text(versioned.note, 0, "Oh\n");
    flush();
    let unsent = crate::wait_until(|shared| {
        let peer = shared.peer.as_ref()?;
        peer.pending_commits(versioned.note).last().copied()
    });

    let (reached, held) = tokio::sync::oneshot::channel();
    let (go, gone) = tokio::sync::oneshot::channel();
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
        held.blocking_recv(),
        Ok(versioned.note),
        "the switch reached writing the note"
    );
    crate::worker::HOLD_DRAINS.store(false, std::sync::atomic::Ordering::SeqCst);
    go.send(()).expect("the switch waits");

    wait_until("finished the switch", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.error.is_none())
    });
    let (text, after) = runtime().block_on(async {
        let peer = harness.outside_peer().await;
        let head = peer
            .remote_head(versioned.note)
            .await
            .expect("the note's head is read")
            .expect("the note has a head");
        let text = peer
            .open::<be_type_text::TextContent>(versioned.note)
            .await
            .expect("the note opens")
            .map(|content| be_type_text::TextBlock::text(&content));
        let after = peer
            .descends(head, unsent)
            .await
            .expect("the history is read");
        (text, after)
    });
    let text = text.expect("the note holds text");
    assert_eq!(
        text.matches("Oh").count(),
        1,
        "the typed line once: {text:?}"
    );
    assert!(
        after,
        "what the switch wrote follows the edit it waited for"
    );
}
