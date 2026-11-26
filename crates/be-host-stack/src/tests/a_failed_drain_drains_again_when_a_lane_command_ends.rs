use super::*;

/// A drain that fails for a reason other than the line (a rotation under way beside it, keys
/// not yet asked for again) keeps the change, and drains again on what may let it through: here
/// a command in the lane ending, as a revoke's rotation does. A held clock never reaches
/// `OUTBOX_RETRY`, so only that signal sends it.
#[test]
fn a_failed_drain_drains_again_when_a_lane_command_ends() {
    let harness = Harness::start().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();
    let before = crate::node(block).and_then(|node| node.head);

    crate::fail_drains(true);
    let drains = crate::wait_until("the drains so far", |shared| Some(shared.drains));
    add(block, 2);
    flush();
    wait_until("the drain the flush started failed", |shared| {
        !shared.draining && shared.drains > drains
    });
    crate::fail_drains(false);
    wait_until("the edit still waits to be sent", |shared| {
        !shared.unsynced().is_empty()
    });

    let (listed, answer) = std::sync::mpsc::channel();
    crate::list_access(block, move |list| {
        let _ = listed.send(list.is_ok());
    });
    assert!(
        answer.recv().expect("the lane answers"),
        "the access was listed"
    );
    wait_until("the edit reached the server", |shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.head.is_some() && node.head != before)
            && counted(shared, block) == Some(3)
            && shared.unsynced().is_empty()
    });
}
