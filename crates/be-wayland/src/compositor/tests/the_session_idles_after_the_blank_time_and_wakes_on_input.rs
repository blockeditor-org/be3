use super::*;

use std::time::Duration;

const BLANK: Duration = Duration::from_secs(600);
const SECOND: Duration = Duration::from_secs(1);

#[test]
fn the_session_idles_after_the_blank_time_and_wakes_on_input() {
    let mut harness = Harness::new();
    harness.compositor().set_blank_after(Some(BLANK));
    let start = harness.context().now() + beui::testing::FRAME_INTERVAL;
    harness.settle();
    let idle = harness.compositor().windows().idle();

    harness.frame_at(start + BLANK - SECOND);
    assert!(!harness.compositor().idle());
    assert_eq!(
        harness.output.as_ref().unwrap().repaint_after,
        SECOND,
        "a frame is asked for when the blank time runs out, not before"
    );

    harness.frame_at(start + BLANK);
    assert!(harness.compositor().idle());
    assert!(idle.get_untracked(), "the UI hears the session went idle");

    harness.frame(vec![Input::PointerMoved(pos2(5.0, 5.0))]);
    let woken = harness.context().now();
    assert!(!harness.compositor().idle(), "input wakes the session");
    assert!(!idle.get_untracked());

    harness.frame_at(woken + BLANK - SECOND);
    assert!(
        !harness.compositor().idle(),
        "the input started the count again"
    );
    harness.frame_at(woken + BLANK);
    assert!(harness.compositor().idle());

    harness.compositor().set_blank_after(None);
    harness.frame(Vec::new());
    harness.context().advance_clock(BLANK * 10);
    harness.frame(Vec::new());
    assert!(!harness.compositor().idle(), "never blanking never idles");
}
