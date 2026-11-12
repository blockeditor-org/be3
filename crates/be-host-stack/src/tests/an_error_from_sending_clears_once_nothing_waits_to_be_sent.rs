use super::*;

/// What went wrong sending (here a refused append, as a reconnect can meet) is not news once the
/// device is connected and nothing waits to be sent: the host stops saying changes could not
/// sync.
#[test]
fn an_error_from_sending_clears_once_nothing_waits_to_be_sent() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    with_shared_mut(|shared| {
        shared.error =
            Some("the server refused (InvalidRequest): the session has a newer owner".into());
    });
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();
    wait_until("the error cleared", |shared| shared.error.is_none());
}
