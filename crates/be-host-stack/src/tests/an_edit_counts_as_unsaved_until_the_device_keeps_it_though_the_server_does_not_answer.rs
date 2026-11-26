use super::*;

/// An edit is unsaved until this device keeps it: once its replica's kept edits hold it, a crash
/// would not lose it, so it is saved though the server's answers are held and the server's order
/// may not hold it yet.
#[test]
fn an_edit_counts_as_unsaved_until_the_device_keeps_it_though_the_server_does_not_answer() {
    let harness = Harness::relayed();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    flush();
    assert_eq!(status().unsaved, 0);

    harness.relay().hold_answers(true);
    add(block, 3);
    assert!(status().unsaved > 0, "an edit not yet taken is unsaved");
    wait_for_count(block, 3);
    wait_until("kept the edit on the device", |held| held.undurable == 0);
    assert_eq!(status().unsaved, 0, "a kept edit is saved, answered or not");
    assert!(
        with_shared(|held| held.peer.as_ref().and_then(|peer| peer.open_edits(block)))
            .flatten()
            .is_some(),
        "its replica's kept edits hold it"
    );

    harness.relay().hold_answers(false);
    assert_eq!(flush(), 0);
}
