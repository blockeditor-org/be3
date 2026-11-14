use super::*;

/// An edit sent before a flush that waits its turn behind a command on a silent server is not
/// saved yet: the flush counts it, so closing asks rather than losing it.
#[test]
fn a_flush_counts_an_edit_that_waits_behind_a_command_on_a_silent_server() {
    let harness = Harness::relayed();
    harness.connect();
    let counter = Uuid::new_v4();
    open(counter, CounterContent::CONTENT_TYPE);
    add(counter, 1);
    wait_for_count(counter, 1);
    assert_eq!(flush(), 0);

    harness.relay().stall();
    let (listed, answered) = std::sync::mpsc::channel();
    list_access(counter, move |access| {
        let _ = listed.send(access.is_ok());
    });
    wait_until("the lane holds the counter", |shared| shared.serving > 0);
    // It needs the counter, which the lane holds, so it waits there.
    add(counter, 2);
    assert!(
        flush() >= 1,
        "the edit waiting behind the command on the silent server is unsaved"
    );

    harness.relay().cut();
    let _ = answered.recv();
    wait_for_count(counter, 3);
    assert_eq!(flush(), 0, "the edit ran once the lane let the counter go");
}
