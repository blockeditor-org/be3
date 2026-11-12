use super::*;

#[test]
fn a_first_connect_that_fails_is_retried_without_moving_a_manual_clock_by_hand() {
    let mut harness = Harness::start().on_driven_clock();
    let address = harness.address();
    harness.stop_server();
    harness.connect();
    crate::wait_until(|shared: &Shared| {
        (!shared.connected && shared.reconnect.is_some()).then_some(())
    });

    harness.start_server(&address);
    crate::wait_until(|shared: &Shared| shared.connected.then_some(()));
}
