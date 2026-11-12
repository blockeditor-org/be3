use super::*;

#[test]
fn a_connection_the_server_drops_is_retried_without_moving_a_manual_clock_by_hand() {
    let mut harness = Harness::start().on_driven_clock();
    let address = harness.address();
    harness.connect();
    crate::wait_until(|shared: &Shared| shared.connected.then_some(()));

    harness.stop_server();
    crate::wait_until(|shared: &Shared| {
        (!shared.connected && shared.reconnect.is_some()).then_some(())
    });

    harness.start_server(&address);
    crate::wait_until(|shared: &Shared| shared.connected.then_some(()));
}
