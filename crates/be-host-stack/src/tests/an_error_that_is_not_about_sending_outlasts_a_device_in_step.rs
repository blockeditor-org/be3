use super::*;

/// Only what went wrong sending clears once the device is in step: another error, such as the
/// server no longer saying how much storage the account uses, stays while edits come and go.
#[test]
fn an_error_that_is_not_about_sending_outlasts_a_device_in_step() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    with_shared_mut(|shared| {
        shared.error = Some(super::super::worker::STORAGE_FAILED.to_owned());
    });
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();
    assert_eq!(
        status().error.as_deref(),
        Some(super::super::worker::STORAGE_FAILED)
    );
}
