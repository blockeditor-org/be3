use super::*;

use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn only_an_input_idle_notification_ignores_inhibitors() {
    let mut harness = Harness::new();
    harness.context.stop_clock();
    let (window, _) = harness.open();
    let _inhibitor = harness.client.inhibit(&window.surface);
    let _idle = harness
        .client
        .idle_notification(TIMEOUT.as_millis() as u32, false);
    harness.settle();

    harness.context.advance_clock(TIMEOUT * 2);
    harness.frame(Vec::new());
    assert!(
        harness.client.received.idle.is_empty(),
        "an inhibitor holds back idle notifications"
    );

    let _input = harness
        .client
        .idle_notification(TIMEOUT.as_millis() as u32, true);
    harness.settle();
    harness.context.advance_clock(TIMEOUT);
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.idle,
        vec![true],
        "an input idle notification only counts input"
    );
}
