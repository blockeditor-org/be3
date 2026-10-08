use super::*;
use crate::state::PING_TIMEOUT;

#[test]
fn a_window_that_stops_answering_pings_is_not_responding_until_it_answers() {
    let mut harness = Harness::new();
    harness.context.stop_clock();
    let (_window, id) = harness.open();
    harness.client.received.ignores_pings = true;
    harness.client.received.pings.clear();

    harness.click(&format!("wayland.window.{}", id.0));

    assert!(
        !harness.client.received.pings.is_empty(),
        "a click on the window pings its client"
    );
    assert!(harness.responding(id), "the client has time to answer");
    assert!(
        harness.output.as_ref().unwrap().repaint_after <= PING_TIMEOUT,
        "the compositor wakes when the answer is due"
    );

    harness.context.advance_clock(PING_TIMEOUT / 2);
    harness.settle();
    assert!(harness.responding(id), "the answer is not due yet");

    harness.context.advance_clock(PING_TIMEOUT / 2);
    harness.settle();
    assert!(
        !harness.responding(id),
        "a client that misses its answer is not responding"
    );

    harness.client.pong();
    harness.settle();
    assert!(harness.responding(id), "the answer clears it");
}
