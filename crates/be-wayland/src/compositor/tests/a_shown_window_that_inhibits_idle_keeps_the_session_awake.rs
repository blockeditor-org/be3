use super::*;

use std::time::Duration;

const BLANK: Duration = Duration::from_secs(60);

#[test]
fn a_shown_window_that_inhibits_idle_keeps_the_session_awake() {
    let mut harness = Harness::new();
    harness.context.stop_clock();
    harness.app.set_blank_after(Some(BLANK));
    let (window, _) = harness.open();
    let inhibitor = harness.client.inhibit(&window.surface);
    harness.settle();

    harness.context.advance_clock(BLANK * 3);
    harness.frame(Vec::new());
    assert!(!harness.app.idle(), "a video playing on screen keeps it on");

    inhibitor.destroy();
    harness.settle();
    harness.context.advance_clock(BLANK);
    harness.frame(Vec::new());
    assert!(
        harness.app.idle(),
        "the count starts when the inhibitor goes away"
    );
}
