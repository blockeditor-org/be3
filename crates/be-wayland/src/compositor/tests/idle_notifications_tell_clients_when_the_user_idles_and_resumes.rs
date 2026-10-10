use super::*;

use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn idle_notifications_tell_clients_when_the_user_idles_and_resumes() {
    let mut harness = Harness::new();
    harness.context.stop_clock();
    let _notification = harness
        .client
        .idle_notification(TIMEOUT.as_millis() as u32, false);
    harness.settle();

    harness
        .context
        .advance_clock(TIMEOUT - Duration::from_millis(1));
    harness.frame(Vec::new());
    assert!(harness.client.received.idle.is_empty());
    harness.context.advance_clock(Duration::from_millis(1));
    harness.frame(Vec::new());
    assert_eq!(harness.client.received.idle, vec![true]);

    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.idle,
        vec![true],
        "idled is sent once"
    );

    harness.frame(vec![Event::PhysicalKey {
        code: 30,
        pressed: true,
    }]);
    assert_eq!(harness.client.received.idle, vec![true, false]);
}
