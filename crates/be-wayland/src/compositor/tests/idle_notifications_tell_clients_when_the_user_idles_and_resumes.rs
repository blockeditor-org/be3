use super::*;

use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn idle_notifications_tell_clients_when_the_user_idles_and_resumes() {
    let mut harness = Harness::new();
    let _notification = harness
        .client
        .idle_notification(TIMEOUT.as_millis() as u32, false);
    let start = harness.context().now() + beui::native::testing::FRAME_INTERVAL;
    harness.settle();

    harness.frame_at(start + TIMEOUT - Duration::from_millis(1));
    assert!(harness.client.received.idle.is_empty());
    harness.frame_at(start + TIMEOUT);
    assert_eq!(harness.client.received.idle, vec![true]);

    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.idle,
        vec![true],
        "idled is sent once"
    );

    harness.frame(vec![physical(30, true)]);
    assert_eq!(harness.client.received.idle, vec![true, false]);
}
