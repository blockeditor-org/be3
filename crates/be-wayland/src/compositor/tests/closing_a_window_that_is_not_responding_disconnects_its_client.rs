use super::*;
use crate::state::PING_TIMEOUT;

#[test]
fn closing_a_window_that_is_not_responding_disconnects_its_client() {
    let mut harness = Harness::new();
    harness.context.stop_clock();
    let (_window, id) = harness.open();
    harness.client.received.ignores_pings = true;
    harness.client.received.pings.clear();

    harness.app.windows().close(id);
    harness.settle();
    assert!(
        !harness.client.disconnected(),
        "a client that may still answer is asked to close"
    );
    assert!(
        !harness.client.received.pings.is_empty(),
        "and pinged, in case it is frozen"
    );

    harness.context.advance_clock(PING_TIMEOUT);
    harness.settle();
    assert!(!harness.responding(id));

    harness.app.windows().close(id);
    harness.settle();

    assert!(
        harness.client.disconnected(),
        "closing a frozen client disconnects it"
    );
    assert!(
        harness.app.windows().list().get_untracked().is_empty(),
        "its window goes"
    );
}
