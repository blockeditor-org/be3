use super::*;

use std::time::Duration;

const BLANK: Duration = Duration::from_secs(60);

#[test]
fn a_wake_from_the_host_counts_as_activity() {
    let mut harness = Harness::new();
    harness.compositor().set_blank_after(Some(BLANK));
    let _notification = harness
        .client
        .idle_notification(BLANK.as_millis() as u32, false);
    harness.settle();
    harness.context().advance_clock(BLANK);
    harness.frame(Vec::new());
    assert!(harness.compositor().idle());
    assert_eq!(harness.client.received.idle, vec![true]);

    harness.compositor().woke();
    harness.frame(Vec::new());
    assert!(
        !harness.compositor().idle(),
        "the input the host swallowed to wake the screens still wakes the session"
    );
    assert_eq!(harness.client.received.idle, vec![true, false]);
}
